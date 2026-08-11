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

fn random_digit(rng: &mut impl Rng) -> char {
    *DIGITS.choose(rng).unwrap()
}

fn random_letter(rng: &mut impl Rng) -> char {
    *LETTERS.choose(rng).unwrap()
}

fn random_alnum(rng: &mut impl Rng) -> char {
    let pool: Vec<char> = DIGITS.iter().chain(LETTERS.iter()).copied().collect();
    *pool.choose(rng).unwrap()
}

fn is_province(ch: char) -> bool {
    PROVINCES.iter().any(|p| p.chars().next() == Some(ch))
}

fn is_alnum_plate_char(ch: char) -> bool {
    DIGITS.contains(&ch) || is_letter(ch)
}

/// 普通车牌后 5 位序号：以数字为主，避免「字母乱炖」导致识别器拒识
fn generate_serial5(rng: &mut impl Rng) -> String {
    let mut s = String::new();
    let roll: f64 = rng.random();
    if roll < 0.55 {
        // 全数字：粤B12345
        for _ in 0..5 {
            s.push(random_digit(rng));
        }
    } else if roll < 0.85 {
        // 首位字母 + 4 数字：粤BA1234
        s.push(random_letter(rng));
        for _ in 0..4 {
            s.push(random_digit(rng));
        }
    } else {
        // 夹杂 1 个字母：粤B12A34
        let letter_pos = rng.random_range(0..5);
        for i in 0..5 {
            if i == letter_pos {
                s.push(random_letter(rng));
            } else {
                s.push(random_digit(rng));
            }
        }
    }
    s
}

/// 新能源后 5 位序号（D/F 之后）：数字为主
fn generate_green_serial5(rng: &mut impl Rng) -> String {
    let mut s = String::new();
    let roll: f64 = rng.random();
    if roll < 0.7 {
        for _ in 0..5 {
            s.push(random_digit(rng));
        }
    } else if roll < 0.9 {
        let letter_pos = rng.random_range(0..5);
        for i in 0..5 {
            if i == letter_pos {
                s.push(random_letter(rng));
            } else {
                s.push(random_digit(rng));
            }
        }
    } else {
        for _ in 0..5 {
            s.push(random_alnum(rng));
        }
    }
    s
}

/// 普通民用号牌：省简称 + 发牌机关代号(字母) + 序号
pub fn generate_blue(rng: &mut impl Rng, length: usize) -> String {
    assert!(length >= 2);
    let mut s = String::new();
    s.push_str(PROVINCES.choose(rng).unwrap());
    s.push(random_letter(rng));
    if length == 7 {
        s.push_str(&generate_serial5(rng));
    } else {
        for _ in 2..length {
            s.push(random_alnum(rng));
        }
    }
    s
}

/// 新能源号牌：省 + 字母 + D/F + 5 位序号
pub fn generate_green_car(rng: &mut impl Rng) -> String {
    let mut s = String::new();
    s.push_str(PROVINCES.choose(rng).unwrap());
    s.push(random_letter(rng));
    s.push(*['D', 'F'].choose(rng).unwrap());
    s.push_str(&generate_green_serial5(rng));
    s
}

/// 是否符合新能源 8 位号牌基本规则（第 3 位 D/F）
pub fn is_valid_green_plate(plate: &str) -> bool {
    let chars: Vec<char> = plate.chars().collect();
    if chars.len() != 8 {
        return false;
    }
    if !is_province(chars[0]) || !is_letter(chars[1]) {
        return false;
    }
    if chars[2] != 'D' && chars[2] != 'F' {
        return false;
    }
    chars[3..].iter().all(|c| is_alnum_plate_char(*c))
}

/// 7 位普通民用号牌基本规则
pub fn is_valid_ordinary_plate(plate: &str) -> bool {
    let chars: Vec<char> = plate.chars().collect();
    if chars.len() != 7 {
        return false;
    }
    if is_letter(chars[0]) {
        return chars[1..].iter().all(|c| is_alnum_plate_char(*c));
    }
    if chars[0] == '使' {
        return chars[1..].iter().all(|c| is_alnum_plate_char(*c));
    }
    if !is_province(chars[0]) || !is_letter(chars[1]) {
        return false;
    }
    const SUFFIX: &[char] = &['学', '警', '挂', '领', '港', '澳'];
    for (i, ch) in chars[2..].iter().enumerate() {
        let last = i + 2 == 6;
        if is_alnum_plate_char(*ch) {
            continue;
        }
        if last && SUFFIX.contains(ch) {
            continue;
        }
        return false;
    }
    true
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
    if n == 8 {
        if !is_valid_green_plate(plate) {
            return Err(
                "新能源车牌格式：省+字母+D/F+5位，如 粤AD12345 / 粤AF12K34".into(),
            );
        }
        return Ok(());
    }
    if !is_valid_ordinary_plate(plate) {
        return Err(
            "普通车牌第 2 位须为字母（发牌机关代号），格式如 粤B12345，不能是 青2XXXXX".into(),
        );
    }
    Ok(())
}
