# 文法一覧

このページはβ07のパーサーが受け付ける文法を説明します。ブロック式、ラベル付きループ、if式、Optionの演算子、比較の連鎖にも対応します。演算子の優先順位と結合はAST構築時に確定します。構文上書けることと、型検査を通ることは別です。例えばブロック内の関数定義は文法上は解析できますが、意味の検査でトップレベル以外の定義を拒否します。実装の原本はリポジトリの `rusk.par` です。

## 表記と字句

以下のEBNFでは、`"..."` はそのまま書く記号・キーワード、`[ ... ]` は省略可能、`{ ... }` は0回以上の繰り返し、`|` は選択、`;` は文法規則の終わりです。文法内の `";"` はRuskのセミコロンを表します。

空白・改行は通常トークンの区切りです。改行で文は終わりません。`//` から行末まではコメントです。ブロックコメントはありません。大文字・小文字を区別します。

| 項目 | 書き方 | 例・制限 |
|---|---|---|
| 識別子 | `[a-zA-Z_][a-zA-Z0-9_]*` | `value`、`Point`、`_count`。日本語や数字から始まる名前は不可 |
| 整数 | `[0-9]+` | `0`、`123`。十進数のみ。桁区切り・16進数・小数は不可 |
| 文字列 | 二重引用符で囲む | `"hello"`。内容はASCII、エスケープはJSON形式 |
| 文字 | 単一引用符で1文字を囲む | `'a'`、`'\n'`。ASCIIの1文字 |
| 真偽値 | `true` / `false` | 出力時の表記は `Yes` / `No` |

負数は単項 `-` を使い `-10` と書きます。符号付き64ビットの範囲は `-9223372036854775808` から `9223372036854775807` です。文字列は `\n`、`\r`、`\t`、`\b`、`\f`、`\\`、`\"`、`\/` とJSONのUnicodeエスケープを解析できますが、復号後の値がASCIIを超えるとエラーです。NULを文字列に入れる場合は `"\u0000"` を使います。文字リテラルは `\n`、`\r`、`\t`、`\0`、`\\`、`\'`、`\"` に対応します。

次のキーワードは変数名などに使わないでください。

```text
include struct enum match fn return extern type symbol let fixed rebindable readonly mutable noescape escape in wt
if else for while loop break continue true false
```

`__Rusk` で始まる型名は処理系内部用のため定義できません。`Int`、`Bool`、`Char`、`Str`、`Unit` は標準型名、`Vec`・`Tuple`・`Option`・`Dict`・`Set` は処理系が扱う型構築子です。`Some`・`None`・`Option`・`Dict`・`Set`・`try_read` は組み込み構築用の関数名として予約しています。`__add__` などの名前は演算フックにも使うため、通常の変数・関数には説明的な名前を使います。

## プログラムと文

```ebnf
program       = { statement } ;
statement     = include | struct_definition | enum_definition | match_expression [ ";" ] | function_definition
              | extern_type | extern_function | extern_symbol
              | let_statement | input | output | return_statement
              | if_statement | for_statement | while_statement
              | loop_statement | break_statement | continue_statement
              | labeled_loop | block [ ";" ] | plain_expression ";" ;
block         = "{" { statement } [ expression ] "}" ;
labeled_loop  = identifier ":" (for_statement | while_statement | loop_statement) ;
include       = "include" string_literal ";" ;
return_statement   = "return" [ expression ] ";" ;
break_statement    = "break" [ identifier ] ";" ;
continue_statement = "continue" [ identifier ] ";" ;
```

実行文はファイルの先頭から順に実行します。`main` 関数を書く必要はありません。型・関数・外部宣言・`include` はトップレベルに置きます。型・関数の定義は使用箇所より後でも構いません。`include` 先の実行文は読み込み位置へ展開されます。

`let`・入出力・`return`・`break`・`continue`・式文・`include` には末尾の `;` が必要です。ブロック、構造体、関数、`if`・ループの閉じ括弧には `;` を付けません。空文 `;` はありません。何もしないブロックには `{}` を使えます。

## 宣言・型・分解

```ebnf
variable_modifier = "fixed" | "rebindable" | "readonly" | "mutable" ;
let_statement = { variable_modifier } "let" binding [ ":" type ] [ initialization expression ] ";" ;
initialization = ":=" | "=" ;
binding       = identifier
              | "(" binding "," binding { "," binding } ")" ;
type          = identifier [ type_arguments ]
              | "(" type "," type { "," type } ")" ;
type_arguments  = "<" type { "," type } ">" ;
type_parameters = "<" identifier { "," identifier } ">" ;
```

| 例 | 意味 |
|---|---|
| `let x := value;` | 型を推論し、値を共有して宣言 |
| `let x = value;` | 型を推論し、値をコピーして宣言 |
| `let x: Int := 1;` | 明示した型と照合して共有 |
| `let x: Int = 1;` | 明示した型と照合してコピー |
| `let x: Int;` | 未初期化で宣言。読む前に `:=` が必要 |
| `let (a, b) := pair;` | タプルを分解して要素を共有 |
| `let (a, (b, _)) = value;` | 全体をコピーし、入れ子で分解 |

型注釈も初期値もない `let x;` は型を決められないためエラーです。分解宣言には初期値が必要です。`_` は初期値を持つ宣言や `for` の束縛で値を読み捨て、変数を作りません。既存変数への代入に使う特別な読み捨て記号ではありません。

`Vec<Int>`、`Vec<Vec<Int>>`、`(Int, Str)`、`Pair<Int, Vec<Str>>` のように型を入れ子にできます。型の括弧はタプル型に使うため、単独の `(Int)` は型として使えません。タプル型・分解パターンは2要素以上です。

`readonly` / `mutable` は[読み取り専用のアクセス](access.md)、`fixed` / `rebindable` の意味と初期化の検査は[名前の結び直しの制限](bindings.md)を参照してください。重複・相反する指定は拒否します。引数には追加で[noescape / escape](escape.md)を指定できます。

## 関数と構造体

```ebnf
function_definition = "fn" identifier [ type_parameters ]
                      "(" parameters ")" [ "->" type ] block ;
parameters          = [ parameter { "," parameter } ] ;
parameter_modifier  = variable_modifier | "noescape" | "escape" ;
parameter           = { parameter_modifier } identifier ":" type ;
struct_definition   = "struct" identifier [ type_parameters ]
                      "{" { struct_field } "}" ;
struct_field        = identifier ":" type ";" ;
```

引数には必ず型を付けます。戻り型を省略すると `Unit` です。引数に分解パターン・初期値・デフォルト値は書けません。構造体のフィールド区切りはカンマではなく `;` です。空の構造体は型検査で拒否します。

```text
struct Box<T> { value: T; }
fn Box<T>(value: T) -> Box<T> {
    return Box::<T> { value: value };
}
```

構造体の宣言だけでは同名の関数は作られません。構築関数が必要なら上のように定義します。型引数の詳しい選択規則は [型と共有](semantics.md) と [β02の機能](beta02.md) を参照してください。

## 入出力

```ebnf
input       = "in" input_item { "," input_item } ";" ;
input_item  = input_name { "," input_name } ":" type ;
input_name  = identifier [ "(" expression_list ")" ] ;
output      = "wt" expression_list ";" ;
expression_list = [ expression { "," expression } ] ;
```

```text
in n, q: Int;                  // 同じ型をまとめて宣言
in s: Str, k: Int;             // 型ごとの組を並べる
in a(n), b(n): Vec<Int>;       // それぞれn個を読み込む
wt n, q, s;
wt;                            // 改行のみ
```

`in` は既存変数の更新ではなく宣言です。同じスコープに同名の変数があるとエラーです。項目は左から処理されるので `in n: Int, a(n): Vec<Int>;` と書けます。括弧の引数は入力関数 `__read__` へ渡す値で、標準の配列入力では各次元に長さを一つ渡します。`in grid(h,w):Vec<Vec<Int>>;`、`in pairs(n):Vec<(Int,Str)>;` と書けます。タプルの入力も可能です。`in a: Vec<Int>;` のような長さの省略はできません。

`wt` は式を左から順に評価して出力し、式の間に空白を一つ、末尾に改行を出します。`wt a, b;` は出力文の2引数で、タプルを作る場合は `wt (a, b);` です。

## 制御構文

```ebnf
if_statement    = "if" condition block [ "else" block ] [ ";" ] ;
for_statement   = "for" binding "in" expression block [ "else" block ] ;
while_statement = "while" condition block [ "else" block ] ;
loop_statement  = "loop" block ;
```

条件の丸括弧は不要ですが、`if (x > 0) { ... }` と付けても構いません。本体の `{ ... }` は省略できません。`else if` はなく、`else { if ... }` と書きます。ループは値を返しません。`if`文とは別に、各分岐をブロック式で書く[if式](beta05.md)があります。

`for` と `while` の `else` は、`break` せずに終了した場合だけ実行します。空の走査や最初から偽の条件でも実行します。`loop` には `else` を付けられません。`break` と `continue` はラベルを省略すると最も内側のループに作用します。`outer: for ...`、`break outer;`、`continue outer;`も使えます。[ブロック式とラベル](blocks-labels.md)を参照してください。

## 式と優先順位

上ほど強く結び付きます。

| 順位 | 構文 | 結合・制限 |
|---|---|---|
| 1 | `a[i]`、`a.field`、`a.method(...)`、`a?!` | 左から連結 |
| 2 | 単項 `+`、`-`、`!` | 右結合 |
| 3 | `*`、`/`、`%` | 左結合 |
| 4 | `+`、`-` | 左結合 |
| 5 | `<?`、`>?` | 左結合・最小値／最大値 |
| 6 | `==`、`!=`、`<`、`<=`、`>`、`>=` | 短絡する比較の連鎖 |
| 7 | `&` | 左結合 |
| 8 | `^` | 左結合 |
| 9 | `\|` | 左結合 |
| 10 | `&&` | 左結合・短絡評価 |
| 11 | `\|\|` | 左結合・短絡評価 |
| 12 | `..` | 1回だけ |
| 13 | `??` | 右結合・不在時だけ右辺を評価 |
| 14 | `:=`、`=`、`+=`、`-=`、`*=`、`/=`、`%=`、`<?=`、`>?=` | 右結合 |

比較はビット演算より強いため、ビットを調べるときは `(mask & bit) != 0` と括弧を付けます。`0 <= x < n` は各隣接項を比較し、偽になった時点で評価を止めます。シフト・ビット反転・ビット複合代入・`++`・`--`・三項演算子は未実装です。

```ebnf
expression = if_expression | match_expression | plain_expression ;
plain_expression = lambda | coalesce [ assignment_operator expression ] ;
if_expression = "if" condition block "else" block ;
coalesce   = range [ "??" coalesce ] ;
lambda     = "(" parameters ")" "=>" expression ;
range      = logical_or [ ".." logical_or ] ;
logical_or = logical_and { "||" logical_and } ;
logical_and = bit_or { "&&" bit_or } ;
bit_or     = bit_xor { "|" bit_xor } ;
bit_xor    = bit_and { "^" bit_and } ;
bit_and    = comparison { "&" comparison } ;
comparison = extreme { comparison_operator extreme } ;
extreme    = addition { ("<?" | ">?") addition } ;
addition   = multiplication { ("+" | "-") multiplication } ;
multiplication = unary { ("*" | "/" | "%") unary } ;
unary      = ("+" | "-" | "!") unary | postfix ;
postfix    = primary { "[" expression "]"
                    | "." identifier | "(" expression_list ")" | "?!" } ;
assignment_operator = ":=" | "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "<?=" | ">?=" ;
comparison_operator = "==" | "!=" | "<" | "<=" | ">" | ">=" ;
```

### 値・呼び出し・構築

```ebnf
primary    = block | array | literal
           | reference [ record_fields ]
           | "(" expression [ tuple_tail ] ")" ;
reference  = identifier [ "::" type_arguments ] { "::" identifier } ;
array      = "[" [ expression ( ";" expression | { "," expression } ) ] "]" ;
tuple_tail = "," expression { "," expression } [ "," ] ;
record_fields = "{" field_value { "," field_value } [ "," ] "}" ;
field_value   = identifier ":" expression ;
literal       = integer_literal | string_literal | character_literal
              | "true" | "false" ;
```

| 形 | 例 | 意味 |
|---|---|---|
| 通常の呼び出し | `f(x, y)` | 名前付き関数を呼ぶ |
| 明示的な型引数 | `f::<Int>(x)` | ジェネリック関数を選ぶ |
| メソッド形式 | `a.f(x)` | `f(a, x)` と同じ |
| フィールド | `p.x` | フィールドを共有して取得 |
| 添字 | `a[i]` | 配列・文字列・タプルの要素を取得 |
| 配列 | `[x, y]` | 同じ型の要素を共有 |
| 繰り返し配列 | `[x; n]` | 各要素を独立にコピー |
| グループ化 | `(x)` | `x` と同じ式 |
| タプル | `(x, y)` | 異なる型も含む2個以上の組 |
| 構造体構築 | `Point { x: 1, y: 2 }` | 全フィールドを名前で指定 |
| 型引数付きの構築 | `Box::<Int> { value: 1 }` | 型引数を明示して構築 |

β04では関数を保持する値を呼べるため、`(f)(x)` や `callbacks[0](x)`、`((x: Int) => x + 1)(3)` が使えます。無名関数の引数名と型は明示します。型・捕捉・コピーの規則は [β04の関数と捕捉](beta04.md) を参照してください。型引数付き呼び出しは `f<Int>(x)` ではなく `f::<Int>(x)` です。メソッド形式での明示的な型引数 `a.f::<Int>()` は使えないため、`f::<Int>(a)` と書きます。

### カンマと括弧の境界

| 場所 | 末尾カンマ |
|---|---|
| タプル式 `(1, 2,)` | 可 |
| 構造体構築 `Point { x: 1, y: 2, }` | 可 |
| 配列 `[1, 2,]` | 不可 |
| 関数の引数 `f(1, 2,)`・定義の仮引数 | 不可 |
| 型引数・型仮引数 `<Int,>`・`<T,>` | 不可 |
| タプル型 `(Int, Str,)`・分解 `(a, b,)` | 不可 |

ブロックの末尾のセミコロンなしの式が結果になります。末尾の式がなければUnitです。文頭のブロックはブロック文として解析するため、演算や呼び出しへつなぐ場合は `({ ... })` と括弧で囲みます。末尾のif/elseもブロックの結果にできます。文法の実装では文と末尾式を左因子化し、CSTの二項演算列からASTで優先順位を組み立てています。

`()`, `(1,)` は単独の値として使えません。`() => expression` は引数なしの無名関数として使えます。`Unit` の値を `()` と表記する構文もありません。

## 外部宣言

外部宣言はC++の実装を接続するための機能です。通常のプログラムでは標準ライブラリの宣言を書き直す必要はありません。

```ebnf
extern_type     = "extern" "type" type "{" properties "}" ;
extern_function = "extern" "fn" identifier "(" parameters ")"
                  [ "->" type ] "{" properties "}" ;
extern_symbol   = "extern" "symbol" identifier "{" properties "}" ;
properties      = { identifier ":" string_literal "," } ;
```

プロパティの値は文字列です。各プロパティには最後の項目を含めて `,` が必要です。型の `cpp_name`、関数の `cpp_name` / `code`、共通コードの `code` などの契約は [開発ガイド](../development/index.md) を参照してください。

## enumとmatch

```ebnf
enum_definition = "enum" identifier [ type_parameters ] "{" [ variants ] "}" ;
variants = variant { "," variant } [ "," ] ;
variant = identifier [ "(" type { "," type } ")" ] ;
match_expression = "match" expression "{" { match_arm } "}" ;
```

variantを持つenumを定義し、`match`で分岐します。matchの対象はenumだけに限らず、基本型・タプル・構造体も使えます。文として使う場合にも網羅性が必要です。[enumとmatch](enums.md)に構築と共有の例があります。

## パターン・ガード・条件

```ebnf
condition = expression | "let" pattern "=" expression ;
pattern = pattern_atom { "|" pattern_atom } ;
pattern_atom = reference [ "(" [ pattern_list ] ")" | struct_pattern_fields ]
             | literal | "-" integer_literal
             | "(" [ pattern [ "," pattern { "," pattern } [ "," ] ] ] ")" ;
pattern_list = pattern { "," pattern } [ "," ] ;
struct_pattern_fields = "{" [ pattern_field { "," pattern_field } [ "," ] ] "}" ;
pattern_field = identifier [ ":" pattern ] | ".." ;
match_arm = pattern [ "if" expression ] "=>"
            (block [ "," ] | expression ",") ;
```

if文・if式・while文は従来の式条件に加え、`let pattern = expression`を条件にできます。`..`は構造体パターンの最後にのみ使えます。単独の`()`はUnit、単独パターンの括弧はグループです。通常のlet・引数・forの束縛文法は拡張しません。詳しいスコープ、評価順、ガード、網羅性は[β06のパターン分岐](beta06.md)を参照してください。

最小・最大演算子`<?`・`>?`と更新`<?=`・`>?=`のコピー・共有・評価順は[専用ページ](minmax.md)を参照してください。

`+=`や`<?=`などの複合代入は一つの演算子です。途中に空白を入れて`+ =`や`<? =`とは書けません。演算子の前後の空白は自由です。
