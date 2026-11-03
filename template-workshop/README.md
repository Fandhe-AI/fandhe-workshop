# template-workshop

教材 repo の雛形（仮称）の置き場です。現時点ではライセンスの表記と、第 2 教材「レビューの頼み方」の例の置き場（[`examples/review-request/`](./examples/review-request/docs/requirements.md)。要件の記録と、教材パックの骨格（Step は 8 つの正しい経路と、模範解答 2 通り。練習データと文書のビューの 5 行を含みます。誤答への応答は今後足します））を置いています。構成（`packs/`・CI・版の指定など）は今後足します。

## 第三者によるビルドとテスト

第 2 教材の例は、エンジンと同じ repo の中にあります。公開された repo を取得するだけで、同じ場所でビルドと教材テストを再現できます。secret や private な依存は要りません。まっさらなコンテナで確かめる手順は、ルートの [README](../README.md) の「まっさらなコンテナでの試験（第三者の再現手順）」にあります（`tools/clean-room.sh run --only pack`）。結果の記録は [docs/testing.md](../docs/testing.md) の 5.452 です。

## ライセンス

この雛形は、エンジンと同じ `MIT OR Apache-2.0` で配布します。本文は同じディレクトリの [LICENSE-MIT](./LICENSE-MIT) と [LICENSE-APACHE](./LICENSE-APACHE) です。

雛形から教材 repo を作るときは、この 2 つのファイルとこの節を残してください。教材 repo の著作権者の表記と教材のライセンスの案内は、雛形の整備の中で別に扱います。
