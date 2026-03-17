use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, SaltString},
    Argon2,
};

fn main() {
    let password = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "ant.design".to_string());
    let salt = SaltString::generate(&mut OsRng);
    match PasswordHash::generate(Argon2::default(), password.as_str(), &salt) {
        Ok(hash) => {
            println!("Password: {}", password);
            println!("Hash: {}", hash);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
