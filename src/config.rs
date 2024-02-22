macro_rules! config {
    ($($name:ident $t:tt $($default:expr)?),+) => {
        lazy_static::lazy_static! {
                $(
                 pub static ref $name: $t = std::env::var(stringify!($name))
                    .unwrap_or_else(|_| {
                        $( if true { return $default.to_string(); } )?
                        panic!("{} is required", stringify!($name));
                    })
                    .parse::<$t>()
                    .unwrap();
                )+
            }
    };
}

config! {
   GH_TOKEN String,

   GITHUB_ACTOR String,

   S3_BUCKET_NAME String,

   S3_KEY String,

   S3_SECRET String,

   S3_ENDPOINT String,

   S3_REGION String ""
}
