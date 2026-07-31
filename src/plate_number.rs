//! 中国车牌号码随机生成与颜色推断

use rand::seq::IndexedRandom;
use rand::Rng;

pub const PROVINCES: &[&str] = &[
    "京", "津", "冀", "晋", "蒙", "辽", "吉", "黑", "沪", "苏", "浙", "皖", "闽", "赣", "鲁",
    "豫", "鄂", "湘", "粤", "桂", "琼", "渝", "川", "贵", "云", "藏", "陕", "甘", "青", "宁",
    "新",
];

pub const DIGITS: &[char] = &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

/// 英文，不含 I、O
pub const LETTERS: &[char] = &[
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'J', 'K', 'L', 'M', 'N', 'P', 'Q', 'R', 'S', 'T',
    'U', 'V', 'W', 'X', 'Y', 'Z',
];

pub fn is_letter(ch: char) -> bool {
    LETTERS.contains(&ch)
}

fn random_alnum(rng: &mut impl Rng) -> char {
    let pool: Vec<char> = DIGITS.iter().chain(LETTERS.iter()).copied().collect();
    *pool.choose(rng).unwrap()
}

pub fn generate_blue(rng: &mut impl Rng, length: usize) -> String {
    let mut s = String::new();
    s.push_str(PROVINCES.choose(rng).unwrap());
    for _ in 1..length {
        s.push(random_alnum(rng));
    }
    s
}

pub fn generate_green_car(rng: &mut impl Rng) -> String {
    let mut s = String::new();
    s.push_str(PROVINCES.choose(rng).unwrap());
    s.push(*LETTERS.choose(rng).unwrap());
    for _ in 0..5 {
        s.push(random_alnum(rng));
    }
    s.push(*DIGITS.choose(rng).unwrap());
    s
}

/// 根据车牌号推断底板颜色（v1 单层常用色）
pub fn infer_bg_color(plate: &str) -> String {
    let n = plate.chars().count();
    if n == 8 {
        return "green_car".into();
    }
    if plate.contains('警') {
        return "white".into();
    }
    if plate.contains('使') {
        return "black_shi".into();
    }
    if plate.contains('领') || plate.contains('港') || plate.contains('澳') {
        return "black".into();
    }
    if plate.contains('学') || plate.contains('挂') {
        return "yellow".into();
    }
    let first = plate.chars().next().unwrap_or('粤');
    if is_letter(first) {
        return "white_army".into();
    }
    "blue".into()
}

/// 随机生成车牌号 + 颜色
pub fn generate_random(rng: &mut impl Rng, prefer_color: Option<&str>) -> (String, String) {
    if let Some(color) = prefer_color {
        let plate = match color {
            "green_car" | "green_truck" => generate_green_car(rng),
            "yellow" => {
                let mut p = generate_blue(rng, 7);
                if rng.random_bool(0.3) {
                    let chars: Vec<char> = p.chars().collect();
                    p = chars[..6].iter().collect::<String>() + "学";
                }
                p
            }
            "white" => {
                let mut p = generate_blue(rng, 7);
                let chars: Vec<char> = p.chars().collect();
                p = chars[..6].iter().collect::<String>() + "警";
                p
            }
            "black" => {
                let p = generate_blue(rng, 7);
                let chars: Vec<char> = p.chars().collect();
                format!(
                    "粤{}{}",
                    chars[1..6].iter().collect::<String>(),
                    if rng.random_bool(0.5) { "港" } else { "澳" }
                )
            }
            "black_shi" => {
                let p = generate_blue(rng, 7);
                let chars: Vec<char> = p.chars().collect();
                format!("使{}", chars[1..].iter().collect::<String>())
            }
            _ => generate_blue(rng, 7),
        };
        return (plate, color.to_string());
    }

    if rng.random_bool(0.35) {
        let plate = generate_green_car(rng);
        return (plate, "green_car".into());
    }

    let plate = generate_blue(rng, 7);
    let color = infer_bg_color(&plate);
    (plate, color)
}

pub fn validate_plate(plate: &str) -> Result<(), String> {
    let n = plate.chars().count();
    if !(7..=8).contains(&n) {
        return Err(format!("车牌长度应为 7 或 8，当前为 {n}"));
    }
    Ok(())
}
