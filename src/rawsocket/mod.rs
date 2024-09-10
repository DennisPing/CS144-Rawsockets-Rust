pub mod socket;
pub mod ip_header;
pub mod tcp_header;
mod ip_flags;
mod tcp_flags;

// Unit test helpers

pub mod test_utils {
    pub fn get_ip_hex() -> &'static str {
        "45000040000040004006d3760a6ed06acc2cc03c"
    }

    pub fn get_tcp_hex() -> &'static str {
        "c6b70050a4269c9300000000b002ffff92970000020405b4010303060101080abb6879f80000000004020000"
    }
}