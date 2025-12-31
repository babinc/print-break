use print_break::print_break;

#[derive(Debug)]
struct User {
    id: u32,
    name: String,
    roles: Vec<String>,
}

fn main() {
    let user_id = 42;
    let name = "ferris";
    let items = vec![1, 2, 3, 4, 5];

    println!("=== Basic types ===");
    print_break!(user_id, name, items);

    let user = User {
        id: 1,
        name: "Alice".to_string(),
        roles: vec!["admin".to_string(), "user".to_string()],
    };

    println!("=== Struct ===");
    print_break!(user);

    // JSON
    let json_response = r#"{"status": "success", "data": {"user_id": 123, "permissions": ["read", "write"]}}"#;
    println!("=== JSON ===");
    print_break!(json_response);

    // TOML
    let toml_config = r#"
[server]
host = "localhost"
port = 8080

[database]
url = "postgres://localhost/db"
max_connections = 10
"#;
    println!("=== TOML ===");
    print_break!(toml_config);

    // YAML
    let yaml_config = r#"
server:
  host: localhost
  port: 8080
database:
  url: postgres://localhost/db
  max_connections: 10
"#;
    println!("=== YAML ===");
    print_break!(yaml_config);

    println!("Done!");
}
