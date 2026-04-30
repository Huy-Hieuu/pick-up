use std::env;

/// Application settings loaded from environment variables.
///
/// In production, these come from the host environment.
/// For local dev, copy `.env.example` to `.env`.
#[derive(Debug, Clone)]
pub struct Settings {
    pub app: AppSettings,
    pub database: DatabaseSettings,
    pub redis: RedisSettings,
    pub email: EmailSettings,
    pub jwt: JwtSettings,
    pub sms: SmsSettings,
    pub momo: MomoSettings,
    pub zalopay: ZaloPaySettings,
    pub s3: S3Settings,
}

#[derive(Debug, Clone)]
pub struct AppSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct DatabaseSettings {
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct RedisSettings {
    pub url: String,
    pub otp_ttl_seconds: i64,
    pub otp_max_attempts: i16,
}

#[derive(Debug, Clone)]
pub struct EmailSettings {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub mail_from: String,
    pub mail_from_name: String,
    pub use_tls: bool,
}

#[derive(Debug, Clone)]
pub struct JwtSettings {
    pub secret: String,
    pub access_ttl_minutes: i64,
    pub refresh_ttl_days: i64,
}

#[derive(Debug, Clone)]
pub struct SmsSettings {
    pub api_key: String,
    pub secret_key: String,
    pub sender: String,
}

#[derive(Debug, Clone)]
pub struct MomoSettings {
    pub partner_code: String,
    pub access_key: String,
    pub secret_key: String,
    pub endpoint: String,
}

#[derive(Debug, Clone)]
pub struct ZaloPaySettings {
    pub app_id: String,
    pub key1: String,
    pub key2: String,
    pub endpoint: String,
}

#[derive(Debug, Clone)]
pub struct S3Settings {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub public_url: String,
}

impl Settings {
    /// Load settings from environment variables.
    /// Call `dotenvy::dotenv()` before this if you want `.env` file support.
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            app: AppSettings {
                host: env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
                port: env::var("APP_PORT")
                    .unwrap_or_else(|_| "8080".into())
                    .parse()?,
            },
            database: DatabaseSettings {
                url: env::var("DATABASE_URL")
                    .map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?,
            },
            redis: RedisSettings {
                url: env::var("REDIS_URL")
                    .unwrap_or_else(|_| "redis://localhost:6379".into()),
                otp_ttl_seconds: env::var("REDIS_OTP_TTL_SECONDS")
                    .unwrap_or_else(|_| "300".into())
                    .parse()?,
                otp_max_attempts: env::var("OTP_MAX_ATTEMPTS")
                    .unwrap_or_else(|_| "5".into())
                    .parse()?,
            },
            email: EmailSettings {
                smtp_host: env::var("SMTP_HOST").unwrap_or_else(|_| "smtp.gmail.com".into()),
                smtp_port: env::var("SMTP_PORT")
                    .unwrap_or_else(|_| "587".into())
                    .parse()?,
                smtp_username: env::var("SMTP_USERNAME").unwrap_or_default(),
                smtp_password: env::var("SMTP_PASSWORD").unwrap_or_default(),
                mail_from: env::var("SMTP_MAIL_FROM")
                    .unwrap_or_else(|_| "noreply@pickup.app".into()),
                mail_from_name: env::var("SMTP_MAIL_FROM_NAME")
                    .unwrap_or_else(|_| "PickUp".into()),
                use_tls: env::var("SMTP_USE_TLS")
                    .unwrap_or_else(|_| "true".into())
                    .parse()
                    .unwrap_or(true),
            },
            jwt: JwtSettings {
                secret: env::var("JWT_SECRET")
                    .map_err(|_| anyhow::anyhow!("JWT_SECRET is required"))?,
                access_ttl_minutes: env::var("JWT_ACCESS_TTL_MINUTES")
                    .unwrap_or_else(|_| "15".into())
                    .parse()?,
                refresh_ttl_days: env::var("JWT_REFRESH_TTL_DAYS")
                    .unwrap_or_else(|_| "30".into())
                    .parse()?,
            },
            sms: SmsSettings {
                api_key: env::var("SMS_API_KEY").unwrap_or_default(),
                secret_key: env::var("SMS_SECRET_KEY").unwrap_or_default(),
                sender: env::var("SMS_SENDER").unwrap_or_else(|_| "PickUp".into()),
            },
            momo: MomoSettings {
                partner_code: env::var("MOMO_PARTNER_CODE").unwrap_or_default(),
                access_key: env::var("MOMO_ACCESS_KEY").unwrap_or_default(),
                secret_key: env::var("MOMO_SECRET_KEY").unwrap_or_default(),
                endpoint: env::var("MOMO_ENDPOINT")
                    .unwrap_or_else(|_| "https://test-payment.momo.vn/v2/gateway/api/create".into()),
            },
            zalopay: ZaloPaySettings {
                app_id: env::var("ZALOPAY_APP_ID").unwrap_or_default(),
                key1: env::var("ZALOPAY_KEY1").unwrap_or_default(),
                key2: env::var("ZALOPAY_KEY2").unwrap_or_default(),
                endpoint: env::var("ZALOPAY_ENDPOINT")
                    .unwrap_or_else(|_| "https://sb-openapi.zalopay.vn/v2/create".into()),
            },
            s3: S3Settings {
                endpoint: env::var("S3_ENDPOINT")
                    .unwrap_or_else(|_| "http://localhost:9000".into()),
                access_key: env::var("S3_ACCESS_KEY").unwrap_or_else(|_| "minioadmin".into()),
                secret_key: env::var("S3_SECRET_KEY").unwrap_or_else(|_| "minioadmin".into()),
                bucket: env::var("S3_BUCKET").unwrap_or_else(|_| "pickup-media".into()),
                public_url: env::var("S3_PUBLIC_URL")
                    .unwrap_or_else(|_| "http://localhost:9000/pickup-media".into()),
            },
        })
    }
}
