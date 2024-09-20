pub mod header;
pub mod ip_flags;
pub mod ip_header;
pub mod rawsocket;
pub mod tcp_flags;
pub mod tcp_header;

// -- Re-export structs for more concise usage

pub use ip_flags::IPFlags;
pub use ip_header::IPHeader;
pub use tcp_flags::TCPFlags;
pub use tcp_header::TCPHeader;

// -- Unit test helpers --

#[cfg(test)]
pub mod test_utils {
    pub fn get_ip_hex() -> &'static str {
        "45000040000040004006d3760a6ed06acc2cc03c"
    }

    pub fn get_tcp_hex() -> &'static str {
        "c6b70050a4269c9300000000b002ffff92970000020405b4010303060101080abb6879f80000000004020000"
    }

    pub fn get_ip_hex_with_payload() -> &'static str {
        "45000592464440002a069de0cc2cc03c0a6ed06a"
    }

    pub fn get_tcp_hex_with_payload() -> &'static str {
        "0050c6b762a01b47a4269e88801000eb71aa00000101080abeb95f0abb687a45"
    }

    pub fn giant_payload() -> &'static str {
        "485454502f312e3120323030204f4b0d0a446174653a205468752c203331204d61722\
        0323032322032303a35383a303220474d540d0a5365727665723a204170616368650d0a55706772616465\
        3a2068322c6832630d0a436f6e6e656374696f6e3a20557067726164652c204b6565702d416c6976650d0\
        a566172793a204163636570742d456e636f64696e672c557365722d4167656e740d0a436f6e74656e742d\
        456e636f64696e673a20677a69700d0a4b6565702d416c6976653a2074696d656f75743d322c206d61783\
        d3130300d0a5472616e736665722d456e636f64696e673a206368756e6b65640d0a436f6e74656e742d54\
        7970653a20746578742f68746d6c3b20636861727365743d5554462d380d0a0d0a316661610d0a1f8b080\
        00000000000036c8f414bc4301085effe8a31e7b65b4151966641da154f5a582f1e43326947d3a424d3d6\
        9f6fbb2be8c1d3e3cdc0f7deabae9bd7faedbd3d42cf833b5c559b8053be9302bdd80ea8cc2a03b202dda\
        b9890a598d8e60fdb97891d1eda183e5033dceea13ec1dd7d59c2d3e48d1ad0b3720982853a0ce3c418e1\
        057909f1937cb78746cd64a0ee83b51e53066d5f3445b5bb407f32fd4a9162265cc61059800e9e57ac140\
        b19eea5c19934e66793017962522e4f5a39943745b9753c57bf600c261d69640afe0fe9390c38aa0ec186\
        f87fa70c1e530a9a1423ac632dae2eae69bfb34e9ad06bcce0f8857afa6693ec791a868130bcf32b8ec98\
        9444281818faa434519582a964e55551d5793045a3bd8e78888fe78ee1a86aaea14f93eecf779df70d359\
        9835414c9139c1e7dac273ff6ec53e4aa1e11ed06de4aaa643eae1d54561167b0019e68229a64731cbc1c\
        2c94d21cac2090926ae7d3882d0fe6521e4ca07dcb7e21adb1fbefec40e87aa8c5c007418605de1374c86\
        cf7e0fcbd5581a5a2cdb14eb6c69d612f394c827c7e60acc625adc3edc8d1e47f7c58d59e5e7a6677e878\
        d9b4b5aba40ff9996e477e71638207dbd89e71aec614004641fc9916693e5f02be7416b85a274e329e9df\
        5452b012c2cbd6ea29330398c9c75061a9d0326b4eb0cda189b177245d0ec9aa7ed08d18b494999ab98d4\
        f0626472f6d3da18a29dbe0ff98a87ade0661203ad7bfe2c44292169ca903495a295dab4eddaa0e062e0e\
        8dc321ce1565c87fef191325133c73dce97d9c3d55e4e015e642ad995d0a45c485d6c330a44b788434b74\
        4d661665ae346df541c04d032e987d33835c8cff78c2cfa990eefc74f63838437625febef0d70de995ef8\
        7e508d79d332f67e8f12565c58f3043cf971592ee4a9b63a4a926562b6408904bc23b01f1d3284d3ad6bd\
        a131c7b3cec92c85beb3a2c627e6f9a2e893c8b4d9dae986f281794408f6e97c49e47441fb237a1175552\
        3dcee675a6ae65cd334f5d01cfebee6f037a35bd8027389b1382047d5a68498e5c0d96c038371d0e660c4\
        5e17b49ded3f9ba44d2ac1405575a3d5cfbc7827984ba282553de7e39fc142e8bd8fb9fb46ad94d180682\
        67f215dbf65ad3d028290d522e88aa48edad37c4c1344e70e53ce404a8c4cf77d60e121c2a2171f57a77d\
        6bb3363248c6bb9e7dc6350495bea3aa59020a3c6efa592bfde45529a86dc2c2a617943bea8a5b5cde1a6\
        dc0c433f2b1001844207e31b130546aeac28ade2115ed7e488c92ea4d125def30d8e287b5692c69bbe46e\
        fc3bb478569649f9251457fba25acca81067e3736c56273177017ad2eb73d46501c039fe80e6641dbc090\
        a00cbe6e247bdd2c70a194c4e4d9cdce372f182815133f4f0ff1902451349f3b9476b70134d181ad1c0b7\
        c8987b9732023a32fa2f1b015509cd97c2b93f1f0ae6dea0eedff47eac0ebe7fdebf323a66eabab47f745\
        2c17899852b76bf9476262fa0bca38531a5406e1ad74"
    }

    pub fn giant_payload_odd() -> &'static str {
        "86def98ab4aa04480dfc00ed6d9326edd05f480b5bc3501311c68af6f37b8e13a03b6d1c904a8aedc4ce\
        f37320bc23897ca909b8ef3bed95e91b02012b1f047e649c1785d91cc8936730a46df6de15f1bf863424d\
        f17ae0f280f7440526fdd7afcf788848f98e491f10946a02ee8e73e32a7e5a4fc74b79dc20012b3c7c82d\
        6f87626c673b1e5e48618c53481de8b226e9c81ed1cf08401f969ea8863a152e0acfea9284b6bd2ea7aaa\
        cf57814d5464a2abe50d120847b21e571f51f6a7c6d448b4504a668beedf0ef6316ebd59d98332aaea0b0\
        6a1b79434462e24be40c8bba0c586180446fca61722c72026374fde5b0bd0e837558aeabd5f87b474f8b0\
        8ee2749b4020faa5ab705b3e65bb99bd51fe5d022256d99b365eb565f257c149888208e650bb189ce13d9\
        2e17df7ef0c80bbb2c3664f9725588b214cffaee21abee8d254a017e08803167dd61b636a3be346e16438\
        2d2b3a9a7342fcc2d4bfd17d680a8fd50c4dbd3bd48e71f1211f9892b5139620456714c90597c34851b8c\
        3d413f68c30f1d57d7d3200c45ff4a838940020cf0c12960825b7cd19818ddb3e17b2803d281660ffc77c\
        fa5c50d33df0a299cdbde7bdac349289598a67ba77d06dd9b4530254dc4005f24f9548d3fa02c528d0975\
        3e64c29cde60554d22181a6064441bdd8257c935152a215767b8a7510cba0c68d060a261d6ac8f174abbb\
        87d4cf8d88b2af05cd2bfe1f5164d53549919815587aec4ef46a0faa2e7b4ba53ed8c0a2c50e8637571a4\
        84222be96c5d75d06cff3df1de4749d2b180299bd0749757f68d7d6d3a8a985d81fac623e83c8e0c88d82\
        5c644521fa5981e243ee82c96e240a9a3af38e2668ea2c52e5bf08c8e71a599f6b75c36dabb35b185adb6\
        8d3c0537dc435496200a194679862b6eb047c81eec4faba6825225f589e6aeed898aaf4909372ac3caffd\
        c40856723416bb6a94b28df3d448cc52e13a8478fb9b6ebf8b1447e423fd6b72980d2df84b08b7b5848f5\
        9ad0c53a7d1bee1c86713e6431ef617d32770921fd330323237009a2ec9a9702560c54a1073105d8cc823\
        240e51a949951920becee7201004a85278a6f5800000d0a300d0a0d0a"
    }
}
