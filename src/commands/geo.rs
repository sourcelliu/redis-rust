// Geo commands for Redis-Rust
// Geographic location commands using Geohash encoding

use crate::protocol::RespValue;
use crate::storage::db::Database;
use crate::storage::types::RedisValue;
use bytes::Bytes;
use std::sync::Arc;
use std::f64::consts::PI;

// Geohash encoding constants
const GEOHASH_LONG_MIN: f64 = -180.0;
const GEOHASH_LONG_MAX: f64 = 180.0;
const GEOHASH_LAT_MIN: f64 = -90.0;
const GEOHASH_LAT_MAX: f64 = 90.0;
const GEOHASH_STEP: usize = 26; // 52 bits total (26 for lat, 26 for long)

// Earth radius in meters
const EARTH_RADIUS_M: f64 = 6372797.560856;

/// Encode latitude and longitude to geohash (52-bit interleaved encoding)
fn geohash_encode(longitude: f64, latitude: f64) -> u64 {
    let mut lat_min = GEOHASH_LAT_MIN;
    let mut lat_max = GEOHASH_LAT_MAX;
    let mut long_min = GEOHASH_LONG_MIN;
    let mut long_max = GEOHASH_LONG_MAX;

    let mut geohash: u64 = 0;

    for i in 0..GEOHASH_STEP {
        // Interleave longitude and latitude bits
        // Even bits: longitude, Odd bits: latitude

        // Longitude bit
        let long_mid = (long_min + long_max) / 2.0;
        if longitude >= long_mid {
            geohash |= 1 << ((GEOHASH_STEP - i - 1) * 2 + 1);
            long_min = long_mid;
        } else {
            long_max = long_mid;
        }

        // Latitude bit
        let lat_mid = (lat_min + lat_max) / 2.0;
        if latitude >= lat_mid {
            geohash |= 1 << ((GEOHASH_STEP - i - 1) * 2);
            lat_min = lat_mid;
        } else {
            lat_max = lat_mid;
        }
    }

    geohash
}

/// Decode geohash to latitude and longitude
fn geohash_decode(geohash: u64) -> (f64, f64) {
    let mut lat_min = GEOHASH_LAT_MIN;
    let mut lat_max = GEOHASH_LAT_MAX;
    let mut long_min = GEOHASH_LONG_MIN;
    let mut long_max = GEOHASH_LONG_MAX;

    for i in 0..GEOHASH_STEP {
        // Longitude bit
        let long_bit = (geohash >> ((GEOHASH_STEP - i - 1) * 2 + 1)) & 1;
        let long_mid = (long_min + long_max) / 2.0;
        if long_bit == 1 {
            long_min = long_mid;
        } else {
            long_max = long_mid;
        }

        // Latitude bit
        let lat_bit = (geohash >> ((GEOHASH_STEP - i - 1) * 2)) & 1;
        let lat_mid = (lat_min + lat_max) / 2.0;
        if lat_bit == 1 {
            lat_min = lat_mid;
        } else {
            lat_max = lat_mid;
        }
    }

    let longitude = (long_min + long_max) / 2.0;
    let latitude = (lat_min + lat_max) / 2.0;

    (longitude, latitude)
}

/// Convert geohash to base32 string (11 characters)
fn geohash_to_string(geohash: u64) -> String {
    const BASE32: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";
    let mut result = String::with_capacity(11);

    for i in 0..11 {
        let idx = ((geohash >> (47 - i * 5)) & 0x1f) as usize;
        result.push(BASE32[idx] as char);
    }

    result
}

/// Calculate distance between two points using Haversine formula
fn haversine_distance(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    let lat1_rad = lat1 * PI / 180.0;
    let lat2_rad = lat2 * PI / 180.0;
    let delta_lat = (lat2 - lat1) * PI / 180.0;
    let delta_lon = (lon2 - lon1) * PI / 180.0;

    let a = (delta_lat / 2.0).sin().powi(2) +
            lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    EARTH_RADIUS_M * c
}

/// Convert distance to specified unit
fn convert_distance(meters: f64, unit: &str) -> f64 {
    match unit.to_lowercase().as_str() {
        "m" => meters,
        "km" => meters / 1000.0,
        "mi" => meters / 1609.34,
        "ft" => meters * 3.28084,
        _ => meters,
    }
}

/// GEOADD key longitude latitude member [longitude latitude member ...]
/// Add geographic positions
pub async fn geoadd(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 4 || (args.len() - 1) % 3 != 0 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'geoadd' command".to_string(),
        );
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Get or create ZSet
    let mut zset = match db_instance.get(&key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => crate::storage::types::ZSet::new(),
    };

    let mut added = 0;
    let mut i = 1;

    while i < args.len() {
        // Parse longitude
        let longitude: f64 = match std::str::from_utf8(&args[i]) {
            Ok(s) => match s.parse() {
                Ok(v) => v,
                Err(_) => return RespValue::Error("ERR invalid longitude".to_string()),
            },
            Err(_) => return RespValue::Error("ERR invalid longitude".to_string()),
        };

        // Validate longitude
        if !(GEOHASH_LONG_MIN..=GEOHASH_LONG_MAX).contains(&longitude) {
            return RespValue::Error("ERR invalid longitude".to_string());
        }

        // Parse latitude
        let latitude: f64 = match std::str::from_utf8(&args[i + 1]) {
            Ok(s) => match s.parse() {
                Ok(v) => v,
                Err(_) => return RespValue::Error("ERR invalid latitude".to_string()),
            },
            Err(_) => return RespValue::Error("ERR invalid latitude".to_string()),
        };

        // Validate latitude
        if !(GEOHASH_LAT_MIN..=GEOHASH_LAT_MAX).contains(&latitude) {
            return RespValue::Error("ERR invalid latitude".to_string());
        }

        let member = Bytes::from(args[i + 2].clone());

        // Encode to geohash
        let geohash = geohash_encode(longitude, latitude);
        let score = geohash as f64;

        // Add to ZSet
        let existed = zset.members.contains_key(&member);
        zset.scores.insert(
            (ordered_float::OrderedFloat(score), member.clone()),
            (),
        );
        zset.members.insert(member, score);

        if !existed {
            added += 1;
        }

        i += 3;
    }

    // Store ZSet
    db_instance.set(key, RedisValue::ZSet(zset));

    RespValue::Integer(added)
}

/// GEOPOS key member [member ...]
/// Get positions (longitude, latitude) of members
pub async fn geopos(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'geopos' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let zset = match db_instance.get(key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => {
            // Return array of nulls
            let result: Vec<RespValue> = (0..args.len() - 1)
                .map(|_| RespValue::Null)
                .collect();
            return RespValue::Array(Some(result));
        }
    };

    let mut result = Vec::new();

    for member_bytes in &args[1..] {
        let member = Bytes::from(member_bytes.clone());

        if let Some(&score) = zset.members.get(&member) {
            let geohash = score as u64;
            let (longitude, latitude) = geohash_decode(geohash);

            result.push(RespValue::Array(Some(vec![
                RespValue::BulkString(Some(longitude.to_string().into_bytes())),
                RespValue::BulkString(Some(latitude.to_string().into_bytes())),
            ])));
        } else {
            result.push(RespValue::Null);
        }
    }

    RespValue::Array(Some(result))
}

/// GEODIST key member1 member2 [unit]
/// Calculate distance between two members
pub async fn geodist(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 {
        return RespValue::Error("ERR wrong number of arguments for 'geodist' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let unit = if args.len() >= 4 {
        std::str::from_utf8(&args[3]).unwrap_or("m")
    } else {
        "m"
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let zset = match db_instance.get(key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Null,
    };

    let member1 = Bytes::from(args[1].clone());
    let member2 = Bytes::from(args[2].clone());

    let score1 = match zset.members.get(&member1) {
        Some(&s) => s,
        None => return RespValue::Null,
    };

    let score2 = match zset.members.get(&member2) {
        Some(&s) => s,
        None => return RespValue::Null,
    };

    let (lon1, lat1) = geohash_decode(score1 as u64);
    let (lon2, lat2) = geohash_decode(score2 as u64);

    let distance_m = haversine_distance(lon1, lat1, lon2, lat2);
    let distance = convert_distance(distance_m, unit);

    RespValue::BulkString(Some(format!("{:.4}", distance).into_bytes()))
}

/// GEOHASH key member [member ...]
/// Get geohash strings of members
pub async fn geohash(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'geohash' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let zset = match db_instance.get(key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => {
            let result: Vec<RespValue> = (0..args.len() - 1)
                .map(|_| RespValue::Null)
                .collect();
            return RespValue::Array(Some(result));
        }
    };

    let mut result = Vec::new();

    for member_bytes in &args[1..] {
        let member = Bytes::from(member_bytes.clone());

        if let Some(&score) = zset.members.get(&member) {
            let geohash = score as u64;
            let hash_str = geohash_to_string(geohash);
            result.push(RespValue::BulkString(Some(hash_str.into_bytes())));
        } else {
            result.push(RespValue::Null);
        }
    }

    RespValue::Array(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geohash_encode_decode() {
        let lon = 13.361389;
        let lat = 38.115556;

        let geohash = geohash_encode(lon, lat);
        let (decoded_lon, decoded_lat) = geohash_decode(geohash);

        // Allow small error due to precision loss
        assert!((decoded_lon - lon).abs() < 0.0001);
        assert!((decoded_lat - lat).abs() < 0.0001);
    }

    #[test]
    fn test_haversine_distance() {
        // Distance between New York and Los Angeles
        let ny_lon = -74.0060;
        let ny_lat = 40.7128;
        let la_lon = -118.2437;
        let la_lat = 34.0522;

        let distance = haversine_distance(ny_lon, ny_lat, la_lon, la_lat);

        // Approximately 3935 km
        assert!((distance / 1000.0 - 3935.0).abs() < 50.0);
    }

    #[tokio::test]
    async fn test_geoadd_geopos() {
        let db = Arc::new(Database::new(16));

        // Add locations
        let result = geoadd(
            &db,
            0,
            vec![
                b"locations".to_vec(),
                b"13.361389".to_vec(),
                b"38.115556".to_vec(),
                b"Palermo".to_vec(),
                b"15.087269".to_vec(),
                b"37.502669".to_vec(),
                b"Catania".to_vec(),
            ],
        )
        .await;
        assert_eq!(result, RespValue::Integer(2));

        // Get positions
        let result = geopos(&db, 0, vec![b"locations".to_vec(), b"Palermo".to_vec()]).await;

        if let RespValue::Array(Some(arr)) = result {
            assert_eq!(arr.len(), 1);
        } else {
            panic!("Expected array result");
        }
    }

    #[tokio::test]
    async fn test_geodist() {
        let db = Arc::new(Database::new(16));

        geoadd(
            &db,
            0,
            vec![
                b"locations".to_vec(),
                b"13.361389".to_vec(),
                b"38.115556".to_vec(),
                b"Palermo".to_vec(),
                b"15.087269".to_vec(),
                b"37.502669".to_vec(),
                b"Catania".to_vec(),
            ],
        )
        .await;

        let result = geodist(
            &db,
            0,
            vec![
                b"locations".to_vec(),
                b"Palermo".to_vec(),
                b"Catania".to_vec(),
                b"km".to_vec(),
            ],
        )
        .await;

        // Distance should be around 166 km
        if let RespValue::BulkString(Some(bytes)) = result {
            let dist_str = String::from_utf8(bytes).unwrap();
            let dist: f64 = dist_str.parse().unwrap();
            assert!((dist - 166.0).abs() < 10.0);
        } else {
            panic!("Expected bulk string result");
        }
    }
}
