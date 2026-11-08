# 辞書・集合と不在を返す操作

β05の `Dict<K,V>` と `Set<T>` はハッシュ型です。走査順は保証しません。キーには `Int`・`Bool`・`Char`・`Str` と、それらを入れ子にしたタプルを使えます。配列・ユーザー構造体・Optionのキー、独自ハッシュ、整列コンテナは未対応です。

## Dict<K,V>

```text
let counts := Dict::<Str, Int>();
counts["apple"] += 1;
counts.set("pear", 3);
wt counts.get("apple") ?? 0;
wt counts.get("missing").is_none(); // 挿入しない
for (key, value) in counts { wt key, value; }
```

| 操作 | 戻り型と動作 | 平均計算量 |
|---|---|---|
| `Dict::<K,V>()` | 空の辞書。型注釈があれば `Dict()` | O(1) |
| `d.len()` / `d.is_empty()` | `Int` / `Bool` | O(1) |
| `d.contains(k)` | `Bool` | O(1) |
| `d.get(k)` | `Option<V>`。見つかった値を共有 | O(1) |
| `d.set(k,v)` | `Unit`。値を共有して登録・結び直し | O(1) + 新規キーのコピー |
| `d[k]` | `V`。なければ `V()` を作って登録 | O(1) + 新規キーのコピー・構築 |
| `d[k] := v` | `V`。`v`を共有して登録・結び直し | setと同じ |
| `d[k] = v` | 格納されたオブジェクトをコピー更新 | 上記の取得 + 値のコピー |
| `d.erase(k)` | 削除できたかを `Bool` で返す | O(1) |
| `d.clear()` | `Unit`。全要素を削除 | O(n) |
| `d.keys()` | コピーしたキーの `Vec<K>` | O(n) + キーのコピー |
| `d.values()` | 値を共有する `Vec<V>` | O(n) |
| `copy(d)` | キーと値を再帰的にコピー | O(n) + 各コピー |

偏った整数キーへの対策として、実行ごとの種を使ってハッシュを混合します。ハッシュ衝突時の検索・追加・削除は最悪O(n)です。文字列・タプルのハッシュと比較にはキーの大きさに応じたコストが掛かります。

添字で値を読む場合、存在するキーについてもコンパイル時に `V()` が使えることを要求します。構造体は型定義だけで構築関数が作られないため、既定値を作れない値には `get`・`set` を使います。`Option::<T>()`、`Dict::<K,V>()`、`Set::<T>()`、空の `Vec<T>` は既定値として作れます。

```text
let d := Dict::<Str, Vec<Int>>();
let row := [1];
d.set("a", row);
let old := d.get("a")?!;
d["a"] := [2];  // oldは以前のrowを保持
let current := d["a"];
d["a"] = [3];   // currentからも3が見える
d.erase("a");   // currentとoldは生存
```

登録時はキーを再帰的にコピーし、値を共有します。readonlyの導入前なので、取得・走査で外へ渡すキーもコピーします。`for (k,v) in d` は `(K,V)` を返し、`k`の更新は辞書に影響せず、`v`の中身の更新は辞書に反映されます。`d = other` は外側の同一性を保ったコピー代入です。

## Set<T>

```text
let visited := Set::<(Int, Int)>();
wt visited.insert((2, 3)); // 新規ならYes
wt visited.insert((2, 3)); // 既存ならNo
wt visited.contains((2, 3));
for point in visited { wt point; }
```

| 操作 | 動作 |
|---|---|
| `Set::<T>()` | 空集合 |
| `s.insert(x)` | コピーして登録。新規なら `true` |
| `s.contains(x)` | 存在の `Bool` |
| `s.get(x)` | コピーしたキーの `Option<T>`。不在ならNone |
| `s.erase(x)` | 削除できたかの `Bool` |
| `s.len()` / `s.is_empty()` | 要素数 / 空か |
| `s.keys()` / `s.values()` | コピーした要素の配列 |
| `s.clear()` | 全削除 |
| `copy(s)` | 要素を再帰的にコピー |

平均・最悪計算量は辞書と同じです。集合の各要素はキーなので、登録・取得・走査でコピーします。空の辞書・集合は条件で偽、それ以外は真です。コンテナ全体の比較・出力は提供しません。

## 走査と構造変更

`for`、`map`、`filter`、`fold`、`to_vec` などの走査に対応します。辞書の要素型は `(K,V)`、集合は `T` です。走査子をコピーすると、同じコンテナの同じ位置を独立に進められます。

新規キー追加、既存キー削除、空でないコンテナのclear、コンテナ全体へのコピー代入は、未終了の走査子を無効化します。次の取得・検査や遅延操作のコールバックの後に位置付きエラーを出します。既存キーの値の更新・結び直し、重複要素のinsert、不在キーのeraseは無効化しません。再ハッシュが起きなくても新規追加は無効化します。走査終了を確認済みの走査子は、追加後も終了状態です。

## 配列find

`a.find(value) -> Option<Int>` は最初に一致した添字を返し、不在ならNoneです。要素型に `__eq__` が必要で、結果はBoolです。計算量は比較回数O(n)。比較中に配列の構造が変わると位置付きエラーです。

```text
wt [3, 1, 3].find(3) ?? (-1); // 0
```

## EOFと構造化入力

`try_read::<T>() -> Option<T>` は `Int`・`Bool`・`Char`・`Str` に対応します。空白を飛ばし、正常なEOFならNone、トークンがあれば通常の入力規則で読みSomeを返します。不正な整数、範囲外、非ASCIIなどは通常どおり実行時エラーです。

```text
let sum := 0;
while true {
    let value := try_read::<Int>();
    if value.is_none() { break; }
    sum += value?!;
}
wt sum;
```

```text
in h, w: Int;
in grid(h, w): Vec<Vec<Int>>;
in pair: (Int, Str);
in edges(3): Vec<(Int, Int)>;
in nested: (Int, (Str, Char));
```

配列の各次元に長さを一つ渡します。要素は左から、入れ子は外側から順に読みます。長さの式は入力前に左から一度ずつ評価します。長さは非負のIntが必要です。タプルの各フィールドも左から読みます。可変長の配列を含むタプル、Option・辞書・集合・構造体の自動入力は提供しません。従来どおり独自の `__read__` を定義して接続できます。
