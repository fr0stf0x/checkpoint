//! Keeps logs in English while errors shown to the user are translated.
//!
//! `tr!` renders a message in the UI language (like `t!`) and also remembers its English
//! rendering for the current thread. `to_en` swaps every remembered translation in a string
//! back to English, so a command's error can be logged in English after the fact.

use std::cell::RefCell;

thread_local! {
    static SEEN: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

pub fn record(user: &str, en: &str) {
    if user == en {
        return;
    }
    SEEN.with(|s| {
        let mut s = s.borrow_mut();
        if s.len() > 256 {
            s.drain(..128);
        }
        s.push((user.to_string(), en.to_string()));
    });
}

/// Replace remembered translations in `s` with their English text (longest first).
pub fn to_en(s: &str) -> String {
    SEEN.with(|v| {
        let mut pairs: Vec<(String, String)> = v.borrow().clone();
        pairs.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        let mut out = s.to_string();
        for (user, en) in pairs {
            if out.contains(&user) {
                out = out.replace(&user, &en);
            }
        }
        out
    })
}

pub fn clear() {
    SEEN.with(|v| v.borrow_mut().clear());
}

/// Translate a user-facing message and remember its English form for the log.
#[macro_export]
macro_rules! tr {
    ($key:expr $(, $name:ident = $val:expr)* $(,)?) => {{
        let user = rust_i18n::t!($key $(, $name = $val)*).to_string();
        let en = rust_i18n::t!($key, locale = "en" $(, $name = $val)*).to_string();
        $crate::i18n::record(&user, &en);
        user
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_back_to_english() {
        clear();
        record("Không đọc được a.sav", "Could not read a.sav");
        record("Game X đang chạy", "X is running");
        assert_eq!(to_en("lỗi: Không đọc được a.sav"), "lỗi: Could not read a.sav");
        assert_eq!(to_en("plain"), "plain");
        clear();
        assert_eq!(to_en("Game X đang chạy"), "Game X đang chạy");
    }
}
