# Library Checker の解答例

Ruskで書いた9問・10解答です。`online-judge-tools/verification-helper` が公開テストケースを取得し、Ruskから生成したC++20の実行結果を検証します。公式のジェネレーター・参照解答・チェッカーを使います。

```bash
python3 tools/test.py --suite public
oj-verify run examples/library_checker/unionfind.test.rusk
```

全テストの共通入口は `python3 tools/test.py` です。問題単位の `oj-verify` はPATH設定後に使えます。導入とCIの構成は [検証ガイド](../../docs/development/verification.md) を参照してください。実行時間・メモリはRuskの共有ポインター実装の影響を受けます。

| 問題 | 解法・検証対象 | 解答 |
|---|---|---|
| [A + B](https://judge.yosupo.jp/problem/aplusb) | 整数入出力 | [aplusb.test.rusk](aplusb.test.rusk) |
| [Many A + B](https://judge.yosupo.jp/problem/many_aplusb) | 64ビット整数と大量入出力 | [many_aplusb.test.rusk](many_aplusb.test.rusk) |
| [Static Range Sum](https://judge.yosupo.jp/problem/static_range_sum) | 累積和 | [static_range_sum.test.rusk](static_range_sum.test.rusk) |
| [Point Add Range Sum](https://judge.yosupo.jp/problem/point_add_range_sum) | 汎用Fenwick Tree | [point_add_range_sum.test.rusk](point_add_range_sum.test.rusk) |
| [Point Add Range Sum](https://judge.yosupo.jp/problem/point_add_range_sum) | 汎用セグメント木 | [point_add_range_sum_segment_tree.test.rusk](point_add_range_sum_segment_tree.test.rusk) |
| [Static RMQ](https://judge.yosupo.jp/problem/staticrmq) | セグメント木 | [staticrmq.test.rusk](staticrmq.test.rusk) |
| [Unionfind](https://judge.yosupo.jp/problem/unionfind) | 構造体・経路圧縮とサイズによる併合 | [unionfind.test.rusk](unionfind.test.rusk) |
| [Z Algorithm](https://judge.yosupo.jp/problem/zalgorithm) | 線形時間の文字列走査 | [zalgorithm.test.rusk](zalgorithm.test.rusk) |
| [Point Set Range Composite](https://judge.yosupo.jp/problem/point_set_range_composite) | 演算として一次関数の合成を渡す汎用セグメント木 | [point_set_range_composite.test.rusk](point_set_range_composite.test.rusk) |
| [Associative Array](https://judge.yosupo.jp/problem/associative_array) | Dict・get・?? | [associative_array.test.rusk](associative_array.test.rusk) |
