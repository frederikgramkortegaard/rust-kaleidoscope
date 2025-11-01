# Kaleidoscope

Rust implementation of the [Kaleidoscope language](https://llvm.org/docs/tutorial/MyFirstLanguageFrontend/index.html) from the LLVM tutorial. This compiler lexes, parses, and generates LLVM IR via [Inkwell](https://github.com/TheDan64/inkwell), then JIT compiles and executes the result.

The implementation covers the core language features from chapters 1-7 of the tutorial: functions, extern declarations, if/then/else, for loops, user-defined operators, and mutable variables. It does not include optimization passes due to Inkwell API differences.

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

# Define ~ with slightly lower precedence than relationals.
def binary~ 9 (LHS RHS)
  !(LHS < RHS | LHS > RHS)
```

User-defined operators are stored in the binary operator precedence table during parsing and compiled as function calls during code generation.

## Example: Mutable Variables

The `mutate.kls` example demonstrates mutable variables from Chapter 7 of the tutorial. Variables are implemented using `alloca`/`load`/`store` instructions instead of SSA φ (phi) nodes:

```bash
cargo run examples/mutate.kls
```

**Source:**

```kaleidoscope
# Function to print a double.
extern printd(x);

# Define '$' for sequencing: as a low-precedence operator that ignores operands
# and just returns the RHS.
def binary$ 1 (x y) y;

def test(x)
  printd(x) $
  x = 4 $
  printd(x);

test(123);
```

**Generated LLVM IR:**

```llvm
define double @test(double %x) {
entry:
  %x1 = alloca double, align 8
  store double %x, ptr %x1, align 8
  %x2 = load double, ptr %x1, align 8
  %calltmp = call double @printd(double %x2)
  store double 4.000000e+00, ptr %x1, align 8
  %binop = call double @"binary$"(double %calltmp, double 4.000000e+00)
  %x3 = load double, ptr %x1, align 8
  %calltmp4 = call double @printd(double %x3)
  %binop5 = call double @"binary$"(double %binop, double %calltmp4)
  ret double %binop5
}
```

**Output:**

```
123
4
Result: 0
```

The example uses the `$` sequencing operator to chain multiple statements. The variable `x` is mutated from 123 to 4, demonstrating that all variables are mutable and stored as stack allocations.

### Local Variables with `var`

The `itefib.kls` example demonstrates the `var` statement for declaring local variables with optional initializers. This allows iterative algorithms like Fibonacci:

```bash
cargo run examples/itefib.kls
```

**Source:**

```kaleidoscope
# Define '$' for sequencing: as a low-precedence operator that ignores operands
# and just returns the RHS.
def binary$ 1 (x y) y;

# Iterative fib.
def fibi(x)
  var a = 1, b = 1, c in
  (for i = 3, i < x in
     c = a + b $
     a = b $
     b = c) $
  b;

# Call it.
fibi(10);
```

The `var` statement declares local variables `a`, `b`, and `c` with optional initializers. The scope of these variables extends to the expression following `in`.

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
│   ├── itefib.kls
│   ├── mandel.kls
│   ├── mutate.kls
│   └── userdefined.kls
└── Cargo.toml
```
