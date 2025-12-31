use print_break::print_break;

#[derive(Debug)]
struct User {
    id: u32,
    name: String,
    roles: Vec<String>,
}

#[derive(Debug)]
enum Status {
    Active,
    Pending(String),
    Error { code: u32, message: String },
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

    // Enums
    let option_some: Option<i32> = Some(42);
    let option_none: Option<i32> = None;
    let result_ok: Result<String, &str> = Ok("success".to_string());
    let result_err: Result<String, &str> = Err("something went wrong");
    let status_active = Status::Active;
    let status_pending = Status::Pending("waiting for approval".to_string());
    let status_error = Status::Error { code: 404, message: "Not found".to_string() };

    println!("=== Enums ===");
    print_break!(option_some, option_none, result_ok, result_err);
    print_break!(status_active, status_pending, status_error);

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
