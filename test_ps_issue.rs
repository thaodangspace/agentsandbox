use std::path::Path;

// Copy the functions to test them
fn sanitize(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn generate_container_name(current_dir: &Path, agent_name: &str) -> String {
    let dir_name = current_dir
        .file_name()
        .and_then(|s| s.to_str())
        .map(sanitize)
        .unwrap_or_else(|| "unknown".to_string());

    let agent_name = sanitize(agent_name);
    let branch_name = "main"; // Simplified for testing
    let timestamp = "2501131430";

    format!("agent-{agent_name}-{dir_name}-{branch_name}-{timestamp}")
}

fn extract_project_name(name: &str) -> String {
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() >= 3 {
        parts[2].to_string()
    } else {
        "unknown".to_string()
    }
}

fn main() {
    // Test cases
    let test_cases = vec![
        ("/home/user/my-project", "claude"),
        ("/home/user/my_complex-project.name", "gemini"),
        ("/home/user/simple", "cursor"),
        ("/home/user/project-with-many-dashes", "qwen"),
    ];

    for (dir, agent) in test_cases {
        let path = Path::new(dir);
        let container_name = generate_container_name(path, agent);
        let extracted_project = extract_project_name(&container_name);
        
        println!("Directory: {}", dir);
        println!("Agent: {}", agent);
        println!("Container name: {}", container_name);
        println!("Extracted project: {}", extracted_project);
        println!("Expected project: {}", path.file_name().unwrap().to_str().unwrap());
        println!("Sanitized expected: {}", sanitize(path.file_name().unwrap().to_str().unwrap()));
        println!("Match: {}", extracted_project == sanitize(path.file_name().unwrap().to_str().unwrap()));
        println!("---");
    }
}
