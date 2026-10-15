//! 入れ子の深さ・大きさの試験入力を作る部品。

/// n 段の入れ子の配列。`closed` が偽なら閉じない（不正な JSON）。
pub fn nested(n: usize, closed: bool) -> Vec<u8> {
    let mut v = vec![b'['; n];
    if closed {
        v.extend(vec![b']'; n]);
    }
    v
}

/// `body` の後ろに空白を足して、ちょうど `total` バイトにする。`body` が長ければそのまま返す。
pub fn pad_to(body: &[u8], total: usize) -> Vec<u8> {
    let mut v = body.to_vec();
    if v.len() < total {
        v.resize(total, b' ');
    }
    v
}
