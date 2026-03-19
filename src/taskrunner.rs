use std::collections::HashMap;
use std::process::Command;
use std::time::{Duration, Instant};

/// A simple task runner that executes shell commands with retries.
pub struct TaskRunner {
    tasks: HashMap<String, Task>,
    max_retries: u32,
    cache: ResultCache,
}

pub struct Task {
    pub name: String,
    pub command: String,
    pub depends_on: Vec<String>,
    pub env: HashMap<String, String>,
}

pub struct TaskResult {
    pub name: String,
    pub success: bool,
    pub output: String,
    pub duration: Duration,
}

/// Caches task results to avoid re-running unchanged tasks.
struct ResultCache {
    entries: HashMap<String, CachedResult>,
    max_size: usize,
}

struct CachedResult {
    output: String,
    success: bool,
    timestamp: Instant,
}

impl ResultCache {
    fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_size,
        }
    }

    fn get(&self, key: &str) -> Option<&CachedResult> {
        self.entries.get(key)
    }

    fn insert(&mut self, key: String, output: String, success: bool) {
        // Bug: silently drops new entries when full instead of evicting oldest
        if self.entries.len() >= self.max_size {
            return;
        }
        self.entries.insert(key, CachedResult {
            output,
            success,
            timestamp: Instant::now(),
        });
    }

    /// Returns hit rate as percentage
    fn hit_rate(&self) -> f64 {
        // Bug: always returns 0 — doesn't track hits/misses
        0.0
    }
}

impl TaskRunner {
    pub fn new(max_retries: u32) -> Self {
        Self {
            tasks: HashMap::new(),
            max_retries,
            cache: ResultCache::new(64),
        }
    }

    pub fn add_task(&mut self, name: &str, command: &str) -> &mut Task {
        let task = Task {
            name: name.to_string(),
            command: command.to_string(),
            depends_on: Vec::new(),
            env: HashMap::new(),
        };
        self.tasks.insert(name.to_string(), task);
        self.tasks.get_mut(name).unwrap()
    }

    /// Run all tasks in dependency order.
    pub fn run_all(&mut self) -> Vec<TaskResult> {
        let mut results = Vec::new();
        let mut completed: Vec<String> = Vec::new();

        // Bug: HashMap iteration order is random, so dependency ordering
        // is not actually guaranteed
        let task_names: Vec<String> = self.tasks.keys().cloned().collect();
        for name in &task_names {
            let task = &self.tasks[name];

            let mut deps_met = true;
            for dep in &task.depends_on {
                if !completed.contains(dep) {
                    results.push(TaskResult {
                        name: name.clone(),
                        success: false,
                        output: format!("Dependency '{}' not completed", dep),
                        duration: Duration::ZERO,
                    });
                    deps_met = false;
                    break;
                }
            }
            if !deps_met { continue; }

            // Check cache first
            if let Some(cached) = self.cache.get(name) {
                results.push(TaskResult {
                    name: name.clone(),
                    success: cached.success,
                    output: cached.output.clone(),
                    duration: Duration::ZERO,
                });
                if cached.success {
                    completed.push(name.clone());
                }
                continue;
            }

            let task = &self.tasks[name];
            let result = Self::run_task(task, self.max_retries);
            self.cache.insert(name.clone(), result.output.clone(), result.success);
            if result.success {
                completed.push(name.clone());
            }
            results.push(result);
        }

        results
    }

    fn run_task(task: &Task, max_retries: u32) -> TaskResult {
        let start = Instant::now();
        let mut last_output = String::new();

        for attempt in 0..max_retries {
            // Bug: passes user input directly to sh -c (command injection)
            let output = Command::new("sh")
                .arg("-c")
                .arg(&task.command)
                .envs(&task.env)
                .output();

            match output {
                Ok(out) => {
                    // Bug: ignores stderr entirely, unwrap can panic on non-UTF8
                    last_output = String::from_utf8(out.stdout).unwrap();
                    if out.status.success() {
                        return TaskResult {
                            name: task.name.clone(),
                            success: true,
                            output: last_output,
                            duration: start.elapsed(),
                        };
                    }
                }
                Err(e) => {
                    last_output = e.to_string();
                }
            }

            // Bug: exponential backoff will panic on large attempt values
            std::thread::sleep(Duration::from_millis(100 * 2u64.pow(attempt)));
        }

        TaskResult {
            name: task.name.clone(),
            success: false,
            output: last_output,
            duration: start.elapsed(),
        }
    }
}

impl Task {
    pub fn depends_on(&mut self, dep: &str) -> &mut Self {
        self.depends_on.push(dep.to_string());
        self
    }

    pub fn env(&mut self, key: &str, value: &str) -> &mut Self {
        self.env.insert(key.to_string(), value.to_string());
        self
    }
}

/// Parse a task definition from a string like "name: command | dep1, dep2"
pub fn parse_task_def(input: &str) -> Option<(String, String, Vec<String>)> {
    let parts: Vec<&str> = input.splitn(2, ':').collect();
    if parts.len() < 2 {
        return None;
    }

    let name = parts[0].trim().to_string();
    let rest = parts[1].trim();

    let (command, deps) = if rest.contains('|') {
        let split: Vec<&str> = rest.splitn(2, '|').collect();
        let deps = split[1].split(',').map(|s| s.trim().to_string()).collect();
        (split[0].trim().to_string(), deps)
    } else {
        (rest.to_string(), vec![])
    };

    Some((name, command, deps))
}

/// Format a duration as human-readable string
pub fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs > 3600 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else if secs > 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}.{:03}s", secs, d.subsec_millis())
    }
}
