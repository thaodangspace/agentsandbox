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

fn extract_project_name_old(name: &str) -> String {
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() >= 3 {
        parts[2].to_string()
    } else {
        "unknown".to_string()
    }
}

fn extract_project_name_new(name: &str) -> String {
    if !name.starts_with("agent-") {
        return "unknown".to_string();
    }
    
    let name_without_prefix = &name[4..]; // Remove "agent-" prefix
    
    // Split the name and try to reconstruct
    let parts: Vec<&str> = name_without_prefix.split('-').collect();
    if parts.is_empty() {
        return "unknown".to_string();
    }
    
    // Get known agent names
    let agents = vec!["claude", "gemini", "codex", "qwen", "cursor"];
    
    // Find which agent this is for
    let mut agent_parts = 0;
    for agent in &agents {
        if name_without_prefix.starts_with(agent) {
            agent_parts = agent.split('-').count(); // In case agent name has dashes
            break;
        }
    }
    
    if agent_parts == 0 {
        return "unknown".to_string();
    }
    
    // The last part should be a timestamp (10 digits)
    if let Some(last_part) = parts.last() {
        if last_part.len() == 10 && last_part.chars().all(|c| c.is_ascii_digit()) {
            // Now we know: agent_parts + project_parts + branch_parts + 1 timestamp = total parts
            let total_parts = parts.len();
            if total_parts <= agent_parts + 1 {
                return "unknown".to_string();
            }
            
            // We need to figure out how many parts are project vs branch
            // For now, let's assume everything between agent and last 2 parts is project
            // (since branch could be 1 part and timestamp is 1 part)
            let project_start = agent_parts;
            let project_end = total_parts - 2; // Exclude branch and timestamp
            
            if project_end > project_start {
                return parts[project_start..project_end].join("-");
            }
        }
    }
    
    "unknown".to_string()
}

// Alternative approach: work backwards from the end
fn extract_project_name_better(name: &str) -> String {
    if !name.starts_with("agent-") {
        return "unknown".to_string();
    }
    
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() < 4 {
        return "unknown".to_string();
    }
    
    // Check if last part is a timestamp (10 digits)
    let timestamp_idx = if let Some(last_part) = parts.last() {
        if last_part.len() == 10 && last_part.chars().all(|c| c.is_ascii_digit()) {
            Some(parts.len() - 1)
        } else {
            None
        }
    } else {
        None
    };
    
    if let Some(ts_idx) = timestamp_idx {
        // Known agent names
        let agents = vec!["claude", "gemini", "codex", "qwen", "cursor"];
        
        // Find the agent (should be at index 1)
        if parts.len() > 1 {
            let potential_agent = parts[1];
            if agents.contains(&potential_agent) {
                // Agent is at index 1, timestamp at ts_idx
                // Everything between index 2 and ts_idx-1 (inclusive) could be project or branch
                // For this approach, let's assume the branch is always the part just before timestamp
                if ts_idx >= 3 {
                    // Project is from index 2 to ts_idx-2 (inclusive)
                    let project_parts = &parts[2..ts_idx-1];
                    if !project_parts.is_empty() {
                        return project_parts.join("-");
                    }
                }
            }
        }
    }
    
    "unknown".to_string()
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
        let extracted_old = extract_project_name_old(&container_name);
        let extracted_new = extract_project_name_new(&container_name);
        let extracted_better = extract_project_name_better(&container_name);
        let expected = sanitize(path.file_name().unwrap().to_str().unwrap());
        
        println!("Directory: {}", dir);
        println!("Container name: {}", container_name);
        println!("Expected project: {}", expected);
        println!("Old extraction: {} ({})", extracted_old, extracted_old == expected);
        println!("New extraction: {} ({})", extracted_new, extracted_new == expected);
        println!("Better extraction: {} ({})", extracted_better, extracted_better == expected);
        println!("---");
    }
}
