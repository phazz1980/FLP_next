//! Built-in blocks of the "Строки" group. Texts are bytes like Arduino `String`:
//! positions and lengths count bytes.

use super::{b, Behavior, Ctx};

const KINDS: [&str; 12] = [
    "ip_to_text",
    "text_to_ip",
    "text_const",
    "text_concat",
    "text_length",
    "num_to_text",
    "text_compare",
    "text_to_num",
    "text_substring",
    "text_find",
    "text_char",
    "text_add_char",
];

pub fn is_text_block(kind: &str) -> bool {
    KINDS.contains(&kind)
}

/// A string block; it has no state, so one type serves all of them.
pub struct TextBlock(pub String);

impl Behavior for TextBlock {
    fn step(&mut self, _: &Ctx, _: &mut [f64]) {}

    fn step_text(&mut self, c: &Ctx, out: &mut [f64], text: &mut [String]) {
        match self.0.as_str() {
            "text_const" => text[0] = c.param_text(0).to_owned(),
            "text_concat" => {
                let count = (c.param(0).max(1.0) as usize).min(16);
                let mut s = c.text(0).to_owned();
                for i in 1..count {
                    s.push_str(c.param_text(1));
                    s.push_str(c.text(i));
                }
                text[0] = s;
            }
            "text_length" => out[0] = c.text(0).len() as f64,
            "num_to_text" => {
                let decimals = c.param(0).max(0.0) as usize;
                text[0] = format!(
                    "{}{:.decimals$}{}",
                    c.param_text(1),
                    c.input(0),
                    c.param_text(2)
                );
            }
            "text_compare" => {
                let (mut a, mut p) = (c.text(0).to_owned(), c.param_text(0).to_owned());
                if c.param(2) != 0.0 {
                    a.make_ascii_lowercase();
                    p.make_ascii_lowercase();
                }
                out[0] = b(match c.param(1) as i32 {
                    1 => a.contains(&p),
                    2 => a.starts_with(&p),
                    3 => a.ends_with(&p),
                    _ => a == p,
                });
            }
            "text_to_num" => {
                // Spaces around are skipped, a decimal comma is accepted.
                let t = c.text(0).trim().replace(',', ".");
                match t.parse::<f64>() {
                    Ok(v) if !t.is_empty() && v.is_finite() => {
                        out[0] = f64::from(v as f32);
                        out[1] = 1.0;
                    }
                    _ => out[1] = 0.0,
                }
            }
            "text_substring" => {
                let bytes = c.text(0).as_bytes();
                let start = (c.param(0).max(0.0) as usize).min(bytes.len());
                let end = if c.param(1) < 0.0 {
                    bytes.len()
                } else {
                    (start + c.param(1) as usize).min(bytes.len())
                };
                text[0] = String::from_utf8_lossy(&bytes[start..end]).into_owned();
            }
            "text_find" => {
                let (hay, sub) = (c.text(0).as_bytes(), c.text(1).as_bytes());
                let from = c.param(0).max(0.0) as usize;
                let pos = if from > hay.len() {
                    None
                } else if sub.is_empty() {
                    Some(from)
                } else {
                    hay[from..]
                        .windows(sub.len())
                        .position(|w| w == sub)
                        .map(|p| p + from)
                };
                out[0] = pos.map_or(-1.0, |p| p as f64);
                out[1] = b(pos.is_some());
            }
            "text_char" => {
                let bytes = c.text(0).as_bytes();
                match usize::try_from(c.input(1) as i64)
                    .ok()
                    .and_then(|i| bytes.get(i))
                {
                    Some(&byte) => {
                        out[1] = f64::from(byte);
                        text[0] = String::from_utf8_lossy(&[byte]).into_owned();
                    }
                    None => {
                        out[1] = 0.0;
                        text[0].clear();
                    }
                }
            }
            "text_add_char" => {
                let mut s = c.text(0).to_owned();
                s.push(char::from((c.input(1) as i64 & 0xFF) as u8));
                text[0] = s;
            }
            "ip_to_text" => {
                let byte = |i| (c.input(i) as i64 & 0xFF) as u8;
                text[0] = format!("{}.{}.{}.{}", byte(0), byte(1), byte(2), byte(3));
            }
            "text_to_ip" => match parse_ip(c.text(0)) {
                Some(parts) => {
                    out[..4].copy_from_slice(&parts.map(f64::from));
                    out[4] = 1.0;
                }
                None => out[4] = 0.0,
            },
            _ => {}
        }
    }
}

/// "a.b.c.d" with every part 0..=255 and digits only.
fn parse_ip(text: &str) -> Option<[u8; 4]> {
    let mut parts = [0u8; 4];
    let mut it = text.split('.');
    for part in &mut parts {
        let s = it.next()?;
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *part = s.parse().ok()?;
    }
    it.next().is_none().then_some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        kind: &str,
        inputs: &[f64],
        texts: &[&str],
        params: &[f64],
        ptexts: &[&str],
    ) -> (Vec<f64>, Vec<String>) {
        let texts: Vec<String> = texts.iter().map(|s| (*s).to_owned()).collect();
        let ptexts: Vec<String> = ptexts.iter().map(|s| (*s).to_owned()).collect();
        let active = vec![true; inputs.len().max(texts.len())];
        let ctx = Ctx {
            now_ms: 0,
            inputs,
            active: &active,
            params,
            texts: &texts,
            param_texts: &ptexts,
        };
        let mut out = vec![0.0; 5];
        let mut text = vec![String::new(); 1];
        TextBlock(kind.to_owned()).step_text(&ctx, &mut out, &mut text);
        (out, text)
    }

    #[test]
    fn concat_joins_with_separator() {
        let (_, t) = run(
            "text_concat",
            &[0.0; 3],
            &["a", "b", "c"],
            &[3.0, 0.0],
            &["", "-"],
        );
        assert_eq!(t[0], "a-b-c");
    }

    #[test]
    fn number_to_text_has_prefix_and_suffix() {
        let (_, t) = run(
            "num_to_text",
            &[21.456],
            &["", "", ""],
            &[1.0, 0.0, 0.0],
            &["", "T=", " C"],
        );
        assert_eq!(t[0], "T=21.5 C");
    }

    #[test]
    fn compare_modes() {
        let c = |a: &str, mode: f64, ic: f64| {
            run("text_compare", &[0.0], &[a], &[0.0, mode, ic], &["on"]).0[0]
        };
        assert_eq!(c("ON", 0.0, 1.0), 1.0);
        assert_eq!(c("ON", 0.0, 0.0), 0.0);
        assert_eq!(c("xonx", 1.0, 0.0), 1.0);
        assert_eq!(c("onx", 2.0, 0.0), 1.0);
        assert_eq!(c("xon", 3.0, 0.0), 1.0);
    }

    #[test]
    fn find_substring_and_char() {
        let (o, _) = run("text_find", &[0.0, 0.0], &["hello", "ll"], &[0.0], &[]);
        assert_eq!((o[0], o[1]), (2.0, 1.0));
        let (o, _) = run("text_find", &[0.0, 0.0], &["hello", "z"], &[0.0], &[]);
        assert_eq!((o[0], o[1]), (-1.0, 0.0));
        let (o, t) = run("text_char", &[0.0, 1.0], &["hello", ""], &[], &[]);
        assert_eq!((o[1], t[0].as_str()), (101.0, "e"));
    }

    #[test]
    fn text_to_number_keeps_last_value_on_error() {
        let (o, _) = run("text_to_num", &[0.0], &[" 3,5 "], &[], &[]);
        assert_eq!((o[0], o[1]), (3.5, 1.0));
        let (o, _) = run("text_to_num", &[0.0], &["abc"], &[], &[]);
        assert_eq!((o[0], o[1]), (0.0, 0.0));
    }

    #[test]
    fn ip_blocks_round_trip() {
        let (_, t) = run("ip_to_text", &[192.0, 168.0, 1.0, 300.0], &[], &[], &[]);
        assert_eq!(t[0], "192.168.1.44");
        let (o, _) = run("text_to_ip", &[0.0], &["10.0.0.7"], &[], &[]);
        assert_eq!(o, [10.0, 0.0, 0.0, 7.0, 1.0]);
        let (o, _) = run("text_to_ip", &[0.0], &["10.0.0"], &[], &[]);
        assert_eq!(o[4], 0.0);
        let (o, _) = run("text_to_ip", &[0.0], &["1.2.3.256"], &[], &[]);
        assert_eq!(o[4], 0.0);
    }

    #[test]
    fn substring_clamps() {
        let (_, t) = run("text_substring", &[0.0], &["hello"], &[1.0, 3.0], &[]);
        assert_eq!(t[0], "ell");
        let (_, t) = run("text_substring", &[0.0], &["hello"], &[9.0, -1.0], &[]);
        assert_eq!(t[0], "");
    }
}
