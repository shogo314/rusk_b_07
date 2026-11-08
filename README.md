# Rusk β 07

競技プログラミング向け言語 **Rusk のβ版・第7段階（rusk_b_07）**を開発・管理するリポジトリです。

完成した [rusk_b_06](https://github.com/shogo314/rusk_b_06) の [最終コミット `899a47f`](https://github.com/shogo314/rusk_b_06/commit/899a47fdc8811dd64e917550ed5459af99a3146e) を出発点に、ソース・設計ノート・ドキュメント・テストを引き継いでいます。Git履歴はこのリポジトリで新しく開始しています。

作者自身にとっての書きやすさと、競技プログラミングで十分な実行速度を重視しています。Python と C++ に慣れた使い方を出発点に、言語を設計しています。

## 現在の状態

β07は開発開始段階です。言語機能と標準APIは完成したβ06を引き継ぎ、変更・副作用・共有・脱出の情報をIRで明示・解析し、一時参照の借用と非脱出スカラーの値保持などへ利用します。任意の引数・ローカル変数の注釈としてreadonly/mutable・fixed/rebindable、引数にはnoescape/escapeも導入する計画です。[β07の要件](design/beta07.md)と[開始点・実装方針](docs/development/beta07.md)を参照してください。関数の効果と戻り値の共有元を求める初期解析を実装中です。注釈の構文・契約検査と、新しい最適化はまだ実装していません。

β06では[パターン分岐・ガード・if let](docs/reference/beta06.md)を追加しています。入れ子のenum、タプル・構造体、定数、orパターン、matchガード、文と式のif let、while letに対応します。[最小・最大演算子](docs/reference/minmax.md)にも対応します。[β06の要件](design/beta06.md)を参照してください。

β05では、[enumとmatch](docs/reference/enums.md)、if式、`Option<T>` と `??`・`?!`、比較の連鎖、ハッシュ型の辞書・集合、多次元配列とタプルの一括入力を追加しています。[要件](design/beta05.md)、[分岐と不在の仕様](docs/reference/beta05.md)、[辞書・集合と入力API](docs/library/beta05.md)、[実装・検証・性能測定](docs/development/beta05.md)を参照してください。

Rust と Parol で、競技プログラミングの基本問題を記述できるプロトタイプを実装しています。C++20 に変換し、基本型の入出力、検査付き整数演算、共有・コピー・代入、短絡論理演算、条件分岐、while / for、再帰・オーバーロードを含む関数定義が使えます。

配列は基本型・構造体・タプル・多次元配列に対応し、一括入力、ソート、二分探索を備えます。ASCII 文字列の添字更新・連結・型変換も使えます。配列の合計、ソートと探索、グリッド BFS、再帰による剰余累乗の実行例は [`examples/prototype/`](examples/prototype/) にあります。

プロトタイプの対象範囲と検証手順は [開発ガイド](docs/development/index.md) を参照してください。型引数を持つ関数・構造体の具体化、タプルと分解、`include` によるファイル分割に対応しています。β03ではバッファ入出力と基本値の一時結果の確保削減、比較用の `--no-opt` を追加しています。[実装方式と性能測定](docs/development/beta03.md) を参照してください。β04では無名関数の捕捉と静的な受け渡し、遅延map/filter、集計・探索・キー付きソート、優先度付きキュー・汎用セグメント木にも対応します。[文法と捕捉](docs/reference/beta04.md) と [API](docs/library/beta04.md) を参照してください。内側の名前付き関数、浮動小数点、readonly、再帰型などは未実装です。設計ノートには検討中・保留中の案も含まれます。

[AtCoderの解答例](examples/atcoder/README.md) は100問を収録しています。整数・文字列処理、全探索、ソート、シミュレーション、DP、グリッド処理を使い、公式サンプル279件を `cargo test --locked --test atcoder` で検証します。

[AOJの36問・41解答](examples/aoj/README.md) と [Library Checkerの10解答](examples/library_checker/README.md) は、`online-judge-tools/verification-helper` で公開テストケースを取得して検証します。[導入と実行方法](docs/development/verification.md) を参照してください。

[ブロック式とラベル付きループ](docs/reference/blocks-labels.md)にも対応します。if式の分岐に文を並べ、ブロック式から外側へbreak/continueできます。

処理系はCST → AST → HIR → LIR → C++の段階に分かれています。LIRでは基本ブロックとジャンプを使います。[変換パイプラインと各段階の確認方法](docs/development/pipeline.md)を参照してください。

## 試す

Rust stable と GCC/Clang の C++20 環境を使います。

```bash
cargo run --locked -- compile examples/aplusb.rusk -o /tmp/aplusb.cpp
g++ -std=c++20 -O2 /tmp/aplusb.cpp -o /tmp/aplusb
printf '10 32\n' | /tmp/aplusb
```

出力は `42` です。[導入手順](docs/getting-started/index.md) と [実装の構成・検証方法](docs/development/index.md) も参照してください。

## テスト

[依存ツールの導入](docs/development/verification.md) 後、共通コマンドでE2E、AtCoderサンプル、サニタイザー、AOJ・Library Checkerの公開テストをまとめて実行できます。CIも同じコマンドを使います。

```bash
python3 tools/test.py
```

手元の確認を短く済ませる場合は `--suite local`、公開テストだけなら `--suite public` を付けます。

## 設計ノート

[design/README.md](design/README.md) を入口として、思想、値の共有とコピー、型、構文、関数、標準ライブラリなどの記録をまとめています。

設計ノートは設計の判断基準や理由を残すもので、実装済みの機能を説明する言語ドキュメントとは区別します。

## ドキュメントの方針

利用者向けドキュメントは、他の人にも理解できる内容を目指し、実装済みの機能に合わせて整備します。[docs/](docs/index.md) の Markdown を MkDocs Material で構築し、[GitHub Pages](https://shogo314.github.io/rusk_b_07/) で公開します。

ローカルでのプレビューと公開の手順は、[ドキュメントの編集と公開](docs/development/documentation.md) を参照してください。

```bash
python -m pip install -r requirements-docs.txt
python -m mkdocs serve
```
