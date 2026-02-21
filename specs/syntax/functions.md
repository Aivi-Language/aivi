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

<!-- quick-info: {"kind":"operator","name":"|>"} -->
Pipelines use `|>`.
<!-- /quick-info -->

<<< ../snippets/from_md/02_syntax/02_functions/block_04.aivi{aivi}

### Choosing the pipe subject (and argument position)

`|>` applies the expression on the right to the value on the left:

<<< ../snippets/from_md/02_syntax/02_functions/block_01.aivi{aivi}


is equivalent to:

<<< ../snippets/from_md/02_syntax/02_functions/block_02.aivi{aivi}


If the right-hand side is already an application, the piped value is passed as the **final** argument:

<<< ../snippets/from_md/02_syntax/02_functions/block_03.aivi{aivi}


is equivalent to:

<<< ../snippets/from_md/02_syntax/02_functions/block_04.aivi{aivi}


This makes pipelines read left-to-right for data flow while keeping ordinary application whitespace-based.

Pipelines often feed directly into `match` branching:

<<< ../snippets/from_md/02_syntax/02_functions/block_05.aivi{aivi}


See also: [Pattern Matching](08_pattern_matching.md) for the `match` operator.

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
