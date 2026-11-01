確認済みの告知（vetted/）の出所。scripts/gen-third-party-licenses.sh が、ライセンスのファイルを持たない crate を補うときだけ読む。

vetted/github.com/Fandhe-AI/fandhe-frontend/
  LICENSE-MIT・LICENSE-APACHE
  出所: https://github.com/Fandhe-AI/fandhe-frontend の commit 25939f573540394cfcc53c45988374226f5ffb5a
  取得日: 2026-11-01（gh api の contents を base64 -d。内容は改変していない）
  対象: Cargo.toml の repository が https://github.com/Fandhe-AI/fandhe-frontend の crate（fandhe-frontend-*）
  更新: 上流の告知が変わったときは、新しい commit から手で取得し直して差し替え、上の sha と日付を更新する

新しい repository を足すときは、同じ形で置き、生成器の vetted_dir に repository の値の完全一致を足す（レビューを要する）。
