mod conn;
mod http;
mod net;
mod tests;

fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn main() {
    println!("2 + 3 = {}", add(2, 3));
}
