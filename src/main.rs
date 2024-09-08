mod rawsocket {
    pub mod header_ip;
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
        assert_eq!(add(-1, 1), 0);
        assert_eq!(add(0, 0), 0);
    }
}

fn main() {
    // Just for sanity, you can print something or call add()
    println!("2 + 3 = {}", add(2, 3));
}
