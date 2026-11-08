# enumとmatch

`enum` は複数の選択肢（variant）のうち一つを保持する型です。値なしのvariantと、位置で区別する複数の値を持つvariantを定義できます。型引数にも対応します。

```rusk
enum Result<T, E> {
    Ok(T),
    Err(E),
}
enum Event {
    Stop,
    Point(Int, Int),
}
let result: Result<Int, Str> := Result::Ok(42);
let point := Event::Point(3, 4);
let stop := Event::Stop;
```

型とvariantの名前は `::` で区切ります。型引数は型注釈、引数の型から推論します。推論できない場合は `Result::<Int,Str>::Err("error")` のように指定します。値なしのvariantは `Event::Stop` または `Event::Stop()` で構築します。定義末尾のカンマは省略できます。enum名は他の型と同じ名前空間に入り、variant名はenumごとに独立しています。

## matchによる分岐

```rusk
enum Event { Stop, Point(Int, Int) }
let event := Event::Point(3, 4);
let answer := match event {
    Event::Point(x, y) => x + y,
    Event::Stop => 0,
};
wt answer;
```

対象は一度だけ評価され、選ばれたarmの本体だけを実行します。すべてのvariantを処理する必要があり、欠けたvariantはコンパイルエラーに表示します。同じvariantの重複や、先のarmによって到達できなくなったarmもエラーです。

対象の型からvariantが決まるので、パターン側では `Point(x,y)` のように型名を省略できます。`_` は残りのvariant全体を処理します。payload内の `_` はその値を無視します。payloadがタプルなら `(a,b)` のように分解できます。束縛した名前のスコープはそのarmだけです。

```rusk
enum Message<T> { Data(T, (Int, Str)), Empty }
let message := Message::Data([1, 2], (3, "row"));
let size := match message {
    Data(row, (_, label)) => {
        wt label;
        row.len()
    }
    _ => 0,
};
wt size;
```

式を直接書くarmは末尾にカンマが必要です。ブロック本体のarmではカンマを省略できます。ブロックの末尾式がそのarmの値になり、末尾式がなければ `Unit` です。値を返して続行するarmの型は一致しなければなりません。`return`・`break`・`continue` で抜けるarmは結果型を制約しません。

文としての `match value { ... }` は結果を捨てます。ブロック式の最後にセミコロンなしで置くと、そのmatchの値がブロックの値になります。算術や呼び出しを後ろに続ける場合は `(match ... { ... })` と括弧で囲みます。文として使う場合も網羅性と結果型を検査します。

## 共有・コピー・代入

構築時にpayloadを共有して保持し、パターンで取り出す際にも共有します。別variantへの更新後も、取り出した値への共有は有効です。

```rusk
enum Cell { Full(Vec<Int>), Empty }
let row := [1];
let cell := Cell::Full(row);
let alias := cell;
let detached = cell;
let held := match cell { Full(x) => x, Empty => [], };
cell = Cell::Empty;
held[0] = 7;
wt row;
wt match alias { Empty => true, Full(_) => false, };
wt match detached { Full(x) => x, Empty => [], };
```

出力は `7`、`Yes`、`1` です。`let detached = cell` と `copy(cell)` は有効なvariantのpayloadを再帰的にコピーします。payloadに同じオブジェクトを複数回渡した場合も、コピー先の各フィールドは独立してコピーします。enum全体への `=` は右辺のスナップショットをコピーし、左辺の外側の同一性を保ってvariantとpayloadを更新します。`:=` は共有先を結び直します。

enumは構造体のフィールド、配列、辞書の値、別のenumのpayloadに使えます。比較・出力・真偽値変換・既定コンストラクターは一般のenumには自動生成しません。必要な操作は型に対応する通常の関数・演算フックとして定義します。辞書キーには使えません。

## Optionも標準のenum

標準ライブラリーの定義は次の形です。

```rusk
enum Option<T> {
    Some(T),
    None,
}
```

利用者のプログラムにはすでに読み込まれているため、この定義を書き直す必要はありません。`Option::Some(x)`、`Option::<Int>::None` と一般のenumと同じ書き方を使えます。互換の `Some(x)`、`Some::<T>(x)`、`None`、`None::<T>()`、`Option::<T>()` も使えます。`match` も通常のenumと同じです。

```rusk
let value := Some(12);
wt match value { Some(n) => n, None => 0, };
```

`??`・`?!`・`value`・`is_some`・`is_none`・真偽値変換はOption向けに残しています。`Dict.get`・`Set.get`・`Vec.find`・`try_read` が返すOptionも同じenum表現です。[OptionのAPI](beta05.md)を参照してください。

## パターンの拡張と制限

β06では、入れ子のenum・定数・構造体・タプル・orパターン・ガードと、if let・while letに対応します。[パターン分岐の仕様](beta06.md)を参照してください。再帰型は引き続き未対応です。
