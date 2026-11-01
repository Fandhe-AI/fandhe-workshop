fandhe-pack の配布物（Apple シリコンの macOS 用）

この配布物には、教材パックを検証・テストするコマンド fandhe-pack が入っています。
Rust の導入は要りません。対応は Apple シリコンの macOS だけです。

署名・公証について

この実行ファイルには Apple の署名も公証もありません（TASK-414 の決定）。
ブラウザなどで落としたファイルには macOS が quarantine の属性を付けるため、
そのままでは起動を止められることがあります。下の手順 3 で、この実行ファイル 1 つだけの属性を外します。

手順

1. 照合する（同じ場所に置いた .sha256 を使います。OK と出れば一致です）

    shasum -a 256 -c fandhe-pack-<版>-aarch64-apple-darwin.tar.gz.sha256

2. 展開する

    tar -xzf fandhe-pack-<版>-aarch64-apple-darwin.tar.gz

3. quarantine の属性を外す（展開してできたディレクトリの中で、実行ファイル 1 つに対して行います）

    xattr -d com.apple.quarantine fandhe-pack

  属性が付いていないときは「No such xattr」と出ますが、無害です。

4. 実行する

    ./fandhe-pack validate <pack-dir>

使い方と出力の読み方は、エンジンのリポジトリの docs/cli-guide.md にあります。

ライセンス

MIT OR Apache-2.0。同梱の LICENSE-MIT と LICENSE-APACHE を参照してください。
