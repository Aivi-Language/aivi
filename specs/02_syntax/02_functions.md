# Functions and Pipes

## 2.1 Application

* Functions are **curried by default**
* Application is by whitespace

<<< ../snippets/from_md/02_syntax/02_functions/block_01.aivi{aivi}

---

## 2.2 Lambdas

`_` denotes a **single-argument lambda**.

<<< ../snippets/from_md/02_syntax/02_functions/block_02.aivi{aivi}

Multi-argument lambdas must be explicit:

<<< ../snippets/from_md/02_syntax/02_functions/block_03.aivi{aivi}

---

## 2.3 Pipes

Pipelines use `|>`.

<<< ../snippets/from_md/02_syntax/02_functions/block_04.aivi{aivi}

---

## 2.4 Usage Examples

### Basic Functions

<<< ../snippets/from_md/02_syntax/02_functions/block_05.aivi{aivi}


### Higher-Order Functions

<<< ../snippets/from_md/02_syntax/02_functions/block_06.aivi{aivi}

### Partial Application

<<< ../snippets/from_md/02_syntax/02_functions/block_07.aivi{aivi}

### Block Pipelines


Pipelines allow building complex data transformations without nested function calls.

<<< ../snippets/from_md/02_syntax/02_functions/block_08.aivi{aivi}

### Expressive Logic: Point-Free Style

Functions can be combined to form new functions without naming their arguments, leading to very concise code.

<<< ../snippets/from_md/02_syntax/02_functions/block_09.aivi{aivi}

### Lambda Shorthand

<<< ../snippets/from_md/02_syntax/02_functions/block_10.aivi{aivi}
