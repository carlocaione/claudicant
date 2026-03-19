use std::collections::HashMap;
use std::process::Command;
use std::time::{Duration, Instant};

/// A simple task runner that executes shell commands with retries.
pub struct TaskRunner {
    tasks: HashMap<String, Task>,
    max_retries: u32,
}

pub struct Task {
    pub name: String,
    pub command: String,
    pub depends_on: Vec<String>,
}

pub struct TaskResult {
    pub name: String,
    pub success: bool,
    pub output: String,
    pub duration: Duration,
}

impl TaskRunner {
    pub fn new(max_retries: u32) -> Self {
        Self {
            tasks: HashMap::new(),
            max_retries,
        }
    }

    pub fn add_task(&mut self, name: &str, command: &str) {
        let task = Task {
            name: name.to_string(),
            command: command.to_string(),
            depends_on: Vec::new(),
        };
        self.tasks.insert(name.to_string(), task);
    }

    /// Run all tasks in dependency order.
    pub fn run_all(&self) -> Vec<TaskResult> {
        let mut results = Vec::new();
        let mut completed: Vec<String> = Vec::new();

        // Bug: HashMap iteration order is random, so dependency ordering
        // is not actually guaranteed
        for (name, task) in &self.tasks {
            for dep in &task.depends_on {
                if !completed.contains(dep) {
                    results.push(TaskResult {
                        name: name.clone(),
                        success: false,
                        output: format!("Dependency '{}' not completed", dep),
                        duration: Duration::ZERO,
                    });
                    continue;
                }
            }

            let result = self.run_task(task);
            if result.success {
                completed.push(name.clone());
            }
            results.push(result);
        }

        results
    }

    fn run_task(&self, task: &Task) -> TaskResult {
        let start = Instant::now();
        let mut last_output = String::new();

        for attempt in 0..self.max_retries {
            // Bug: passes user input directly to sh -c (command injection)
            let output = Command::new("sh")
                .arg("-c")
                .arg(&task.command)
                .output();

            match output {
                Ok(out) => {
                    // Bug: ignores stderr entirely
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
