# Kaleidoscope

Rust implementation of the [Kaleidoscope language](https://llvm.org/docs/tutorial/MyFirstLanguageFrontend/index.html) from the LLVM tutorial. This compiler lexes, parses, and generates LLVM IR via [Inkwell](https://github.com/TheDan64/inkwell), then JIT compiles and executes the result.

The implementation covers the core language features from chapters 1-6 of the tutorial: functions, extern declarations, if/then/else, for loops, and user-defined operators. It does not include optimization passes due to Inkwell API differences.

## Build

```bash
cargo build --release
```

## Design

The compiler is structured around three context objects that encapsulate each compilation phase:

- **LexerContext**: Tokenizes the entire input upfront into a vector, then provides peek/consume methods for navigation

- **ParserContext**: Parses tokens into an AST, storing all function definitions. Top-level expressions are wrapped as anonymous `_top_level_expr` functions (this is per [the Kaleidoscope tutorial design](https://llvm.org/docs/tutorial/MyFirstLanguageFrontend/LangImpl02.html), not a personal design decision)

- **CodegenContext**: Owns the LLVM context, builder, module, and variable map. Generates IR for all functions and places the last top-level expression into a `main()` function for JIT execution

Each context uses an in-place mutation pattern.

The compiler uses LLVM's JIT execution engine to compile the generated IR to native code and execute it immediately, without writing object files or linking. Extern functions are registered with the JIT via an FFI registry that maps function names to native Rust function pointers.

## Example: For Loops

The `for.kls` example demonstrates for loop code generation. The loop compiles to LLVM IR with PHI nodes for the loop variable, showing how control flow is lowered to SSA form:

```bash
cargo run examples/for.kls
```

**Source:**

```kaleidoscope
extern putchard(char);

def printstar(n)
  for i = 1, i < n, 1.0 in
    putchard(42);

printstar(10);
```

**Generated LLVM IR:**

```llvm
define double @printstar(double %n) {
entry:
  br label %loop

loop:                                             ; preds = %loop, %entry
  %i = phi double [ 1.000000e+00, %entry ], [ %nextvar, %loop ]
  %calltmp = call double @putchard(double 4.200000e+01)
  %nextvar = fadd double %i, 1.000000e+00
  %cmptmp = fcmp ult double %i, %n
  %booltmp = uitofp i1 %cmptmp to double
  %loopcond = fcmp one double %booltmp, 0.000000e+00
  br i1 %loopcond, label %loop, label %afterloop

afterloop:                                        ; preds = %loop
  ret double 0.000000e+00
}
```

**Output:**

```
**********
Result: 0
```

## Example: User-Defined Operators

The `userdefined.kls` example demonstrates user-defined operators from Chapter 6 of the tutorial. It shows how to define custom unary and binary operators with specified precedence levels:

```bash
cargo run examples/userdefined.kls
```

**Source:**

```kaleidoscope
# Logical unary not.
def unary!(v)
  if v then
    0
  else
    1

# Define > with the same precedence as <.
def binary> 10 (LHS RHS)
  RHS < LHS

# Binary "logical or", (note that it does not "short circuit")
def binary| 5 (LHS RHS)
  if LHS then
    1
  else if RHS then
    1
  else
    0

# Define = with slightly lower precedence than relationals.
def binary= 9 (LHS RHS)
  !(LHS < RHS | LHS > RHS)
```

User-defined operators are stored in the binary operator precedence table during parsing and compiled as function calls during code generation.

## Example: Mandelbrot Set

The `mandel.kls` example is the full Mandelbrot set renderer from the tutorial. It demonstrates recursive functions, nested for loops, and calling extern functions to render ASCII graphics:

```bash
cargo run examples/mandel.kls
```

**Output:**

```
*******************************************************************************
*******************************************************************************
****************************************++++++*********************************
************************************+++++...++++++*****************************
*********************************++++++++.. ...+++++***************************
*******************************++++++++++..   ..+++++**************************
******************************++++++++++.     ..++++++*************************
****************************+++++++++....      ..++++++************************
**************************++++++++.......      .....++++***********************
*************************++++++++.   .            ... .++**********************
***********************++++++++...                     ++**********************
*********************+++++++++....                    .+++*********************
******************+++..+++++....                      ..+++********************
**************++++++. ..........                        +++********************
***********++++++++..        ..                         .++********************
*********++++++++++...                                 .++++*******************
********++++++++++..                                   .++++*******************
*******++++++.....                                    ..++++*******************
*******+........                                     ...++++*******************
*******+... ....                                     ...++++*******************
*******+++++......                                    ..++++*******************
*******++++++++++...                                   .++++*******************
*********++++++++++...                                  ++++*******************
**********+++++++++..        ..                        ..++********************
*************++++++.. ..........                        +++********************
******************+++...+++.....                      ..+++********************
*********************+++++++++....                    ..++*********************
***********************++++++++...                     +++*********************
*************************+++++++..   .            ... .++**********************
**************************++++++++.......      ......+++***********************
****************************+++++++++....      ..++++++************************
*****************************++++++++++..     ..++++++*************************
*******************************++++++++++..  ...+++++**************************
*********************************++++++++.. ...+++++***************************
***********************************++++++....+++++*****************************
***************************************++++++++********************************
*******************************************************************************
*******************************************************************************
*******************************************************************************
*******************************************************************************
*******************************************************************************

Result: 0
```

## Project Structure

```
.
├── ast.rs          # AST definitions
├── lexer.rs        # Tokenizer
├── parser.rs       # Parser
├── codegen.rs      # LLVM IR generation
├── externs.rs      # FFI registry for native functions
├── main.rs         # Entry point
├── examples/
│   ├── for.kls
│   ├── mandel.kls
│   └── userdefined.kls
└── Cargo.toml
```
