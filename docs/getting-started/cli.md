# コマンドライン

以下の`rusk`は、ビルドした`./target/release/rusk`を表します。入力ファイルは`compile`の直後に指定します。

```text
rusk compile FILE [-o OUTPUT] [--no-opt] [--emit cst|ast|hir|lir|cpp]
rusk dependencies FILE
rusk --help
```

## C++を生成する

```bash
rusk compile main.rusk -o main.cpp
```

| オプション | 意味 |
|---|---|
| `-o PATH` / `--output PATH` | 出力先。省略時はカレントディレクトリの`a.cpp` |
| `--emit STAGE` | 出力する段階。省略時は`cpp` |
| `--no-opt` | 標準で有効な最適化を無効化して比較する |
| `--help` / `-h` | 処理系の使い方を表示する（単独で指定） |

`compile`はC++コンパイラを起動しません。実行ファイルの作成は別途`g++`などで行います。構文・型検査などに失敗すると診断を標準エラーへ表示し、正常終了しません。以前生成したファイルが残る場合があるため、失敗後に古い生成物を実行しないようにしてください。

## 中間表現を調べる

```bash
rusk compile main.rusk --emit hir -o /tmp/main.hir
rusk compile main.rusk --emit lir -o /tmp/main.lir
```

| 段階 | 主な内容 |
|---|---|
| `cst` | 入力ソースの構文構造 |
| `ast` | ASTの構造。演算子の優先順位などを反映 |
| `hir` | 型や参照先を解決した中間表現 |
| `lir` | 基本ブロックと分岐を持つ中間表現 |
| `cpp` | 実行用のC++ソース |

`--emit`を変えても既定の出力ファイル名は`a.cpp`です。CST・ASTの出力は調査用であり、型検査が成功したことの代わりにはなりません。各段階の役割は[変換パイプライン](../development/pipeline.md)に説明しています。

## includeの依存先を調べる

```bash
rusk dependencies main.rusk
```

読み込み対象のソースパスを出力します。依存ファイルの確認に使うコマンドで、プログラム全体の型検査や実行は行いません。ファイル分割の書き方は[include](../reference/beta02.md)を参照してください。
