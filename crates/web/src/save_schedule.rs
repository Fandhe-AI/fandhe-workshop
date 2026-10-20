//! 変更の後の遅延保存の判定（純関数。TASK-318、SAVE-7）。
//!
//! 呼び出し元は `glue` の保存の通知先で、ホストが「履歴が変わった」と通知するたびに [`SaveDebounce::on_change`] で
//! 遅延を求めて `set_timeout` を予約し、タイマーが発火したら [`SaveDebounce::on_fire`] で書き出すかを決める。
//! 連続する変更を 1 回の書き出しに畳む（打鍵ごとに履歴の全体を書き出す費用を避ける）一方、変更が続いても
//! 最初の変更から [`SAVE_MAX_WAIT_MS`] を超えて待たない。時刻は呼び出し元が渡し、ここは時計も I/O も持たない。
//!
//! 古いタイマーは `clearTimeout` で消さず、token で捨てる（予約した token が最新でなければ無視する）。
//!
//! 定数は実装の値であり、試験の期待値にしない（SAVE-7 が要求するのは変更から 1 秒以内という結果であって、
//! 遅延の長さではない）。

/// 最後の変更からこの時間だけ待って書き出す（ミリ秒）。実装の値。期待にしない（SAVE-7）。
pub const SAVE_DEBOUNCE_MS: f64 = 200.0;

/// 最初の変更からこの時間を超えて待たない（ミリ秒）。実装の値。期待にしない（SAVE-7）。
pub const SAVE_MAX_WAIT_MS: f64 = 500.0;

/// 予約すべきタイマー。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Schedule {
    /// 発火のときに [`SaveDebounce::on_fire`] へ返す印。
    pub token: u64,
    /// 予約する遅延（ミリ秒。0 以上）。
    pub delay_ms: f64,
}

/// 発火したときの判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fire {
    /// 書き出す。
    Save,
    /// 古いタイマー。何もしない。
    Ignore,
}

/// 変更の畳み込みの状態。
#[derive(Debug, Default)]
pub struct SaveDebounce {
    /// 最初の未保存の変更の時刻。保留が無ければ `None`。
    first: Option<f64>,
    /// 最後に発行した token。
    token: u64,
}

impl SaveDebounce {
    /// 空の状態。
    pub const fn new() -> Self {
        Self {
            first: None,
            token: 0,
        }
    }

    /// 変更を受け、新しいタイマーの予約を返す（以前の予約は token の不一致で無効になる）。
    pub fn on_change(&mut self, now: f64) -> Schedule {
        let first = *self.first.get_or_insert(now);
        self.token = self.token.wrapping_add(1);
        let rest = first + SAVE_MAX_WAIT_MS - now;
        Schedule {
            token: self.token,
            delay_ms: SAVE_DEBOUNCE_MS.min(rest).max(0.0),
        }
    }

    /// タイマーの発火を受ける。最新の token だけが書き出しになり、状態は空に戻る。
    pub fn on_fire(&mut self, token: u64) -> Fire {
        if self.first.is_some() && token == self.token {
            self.first = None;
            Fire::Save
        } else {
            Fire::Ignore
        }
    }

    /// 予約を無効にする（即時の保存が済んだとき）。畳み込みの状態も空に戻し、待っていた古いタイマーは token の不一致で捨てる。
    pub fn cancel(&mut self) {
        self.first = None;
        self.token = self.token.wrapping_add(1);
    }

    /// 発火を待っている変更があるか。
    pub fn is_scheduled(&self) -> bool {
        self.first.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 無効にすると古いタイマーは書かず_畳み込みも空に戻る() {
        let mut d = SaveDebounce::new();
        let s = d.on_change(1000.0);
        d.cancel();
        assert!(!d.is_scheduled());
        assert_eq!(d.on_fire(s.token), Fire::Ignore);
        let s2 = d.on_change(5000.0);
        assert_eq!(
            s2.delay_ms, SAVE_DEBOUNCE_MS,
            "最初の変更の時刻は持ち越さない"
        );
    }

    #[test]
    fn 一回の変更は遅延ののち一回だけ発火する() {
        let mut d = SaveDebounce::new();
        let s = d.on_change(1000.0);
        assert_eq!(s.delay_ms, SAVE_DEBOUNCE_MS);
        assert!(d.is_scheduled());
        assert_eq!(d.on_fire(s.token), Fire::Save);
        assert!(!d.is_scheduled());
        assert_eq!(d.on_fire(s.token), Fire::Ignore);
    }

    #[test]
    fn 連続する変更は最後の一回に畳まれる() {
        let mut d = SaveDebounce::new();
        let a = d.on_change(0.0);
        let b = d.on_change(50.0);
        let c = d.on_change(100.0);
        assert_eq!(d.on_fire(a.token), Fire::Ignore);
        assert_eq!(d.on_fire(b.token), Fire::Ignore);
        assert_eq!(d.on_fire(c.token), Fire::Save);
    }

    #[test]
    fn 変更が続いても最初の変更から最大の待ちで発火する() {
        let mut d = SaveDebounce::new();
        let mut now = 0.0;
        let mut last = d.on_change(now);
        let mut fire_at = now + last.delay_ms;
        while now + 50.0 < fire_at {
            now += 50.0;
            last = d.on_change(now);
            fire_at = now + last.delay_ms;
        }
        assert!(fire_at <= SAVE_MAX_WAIT_MS);
        assert_eq!(d.on_fire(last.token), Fire::Save);
    }

    #[test]
    fn 最大の待ちを過ぎた変更の遅延は_0_になる() {
        let mut d = SaveDebounce::new();
        d.on_change(0.0);
        assert_eq!(d.on_change(SAVE_MAX_WAIT_MS + 100.0).delay_ms, 0.0);
    }

    #[test]
    fn 発火の後の変更は新しい窓を始める() {
        let mut d = SaveDebounce::new();
        let a = d.on_change(0.0);
        assert_eq!(d.on_fire(a.token), Fire::Save);
        let b = d.on_change(10_000.0);
        assert_eq!(b.delay_ms, SAVE_DEBOUNCE_MS);
        assert_eq!(d.on_fire(b.token), Fire::Save);
    }

    #[test]
    fn 変更の無い発火は無視する() {
        let mut d = SaveDebounce::new();
        assert_eq!(d.on_fire(0), Fire::Ignore);
    }
}
