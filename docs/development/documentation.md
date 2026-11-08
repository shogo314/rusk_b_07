# ドキュメントの編集と公開

Markdown を `docs/` に置き、MkDocs Material でサイトを生成します。サイトの構成はリポジトリ直下の `mkdocs.yml` で管理します。

## ローカルで準備する

リポジトリ直下で次を実行します。

```bash
python3 -m venv .venv-docs
source .venv-docs/bin/activate
python -m pip install -r requirements-docs.txt
```

## プレビューとビルド

```bash
python -m mkdocs serve
```

表示されたローカル URL をブラウザで開きます。公開前の検査は次のコマンドで行います。

```bash
python -m mkdocs build --strict
```

生成先は `site/` です。生成物と仮想環境は Git に含めません。

## ページを追加する

`docs/` に Markdown ファイルを追加し、`mkdocs.yml` の `nav` に登録します。相対リンクで関連ページへつなぎ、追加後に strict ビルドを実行します。

利用者向けの例は、次の順で確認して掲載します。

1. 単独で動くプログラムには、必要な入力と期待する出力を添える。
2. 現在の処理系でC++へ変換し、C++20としてコンパイルする。
3. 掲載した入力で実行し、標準出力が説明どおりであることを確認する。
4. 構文断片・未定義の名前を含む例・意図的なエラー例は、そのことが分かる説明を付ける。

言語の説明は機能別のページへ置き、版ごとの実装記録・測定値は開発ガイドへ置きます。実装済みの機能と将来の設計を混在させず、共有・コピー・評価順が重要なAPIにはその違いを記載します。

strictビルドはサイトの構成やリンクを検査しますが、Ruskコードを実行するものではありません。掲載例の実行確認は別途必要です。

## GitHub Pages へ公開する

公開用ワークフローは `.github/workflows/docs.yml` にあります。`main` にドキュメント関連の変更をpushすると、strictビルドの成功後に自動で公開します。対象は `docs/**`、`mkdocs.yml`、`requirements-docs.txt`、公開ワークフロー自身です。プルリクエストではビルド検査だけを行います。

必要なら Actions の **Documentation** ワークフローを `main` に対して手動実行して再公開できます。公開ジョブは同時に実行しないよう制御しています。

初回公開前に GitHub の **Settings → Pages → Build and deployment → Source** を **GitHub Actions** に設定します。想定する公開先は `https://shogo314.github.io/rusk_b_07/` です。

公開後のサイトは [Rusk β 07](https://shogo314.github.io/rusk_b_07/) で確認できます。

参考：[MkDocs の公開ガイド](https://www.mkdocs.org/user-guide/deploying-your-docs/)、[GitHub Pages のカスタムワークフロー](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages)。
