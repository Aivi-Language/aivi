# AIVI for TypeScript Developers

If you are coming from TypeScript, AIVI will feel both familiar and radically different. It shares the goal of static typing and structural data, but enforces purity and immutability much more strictly.

## Key Differences

| Feature | TypeScript | AIVI |
| :--- | :--- | :--- |
| **Data** | Mutable by default (unless `readonly`) | Immutable by default |
| **Null/Undefined** | `null`, `undefined` | `Option A` (None \| Some A) |
| **Errors** | `throw`, `try/catch` | `Result E A` (Err E \| Ok A) |
| **Loops** | `for`, `while`, `map/reduce` | Recursion, `fold`, Generators |
| **Async** | `Promise`, `async/await` | `Effect`, `fiber` hierarchy |
| **Strings** | `+` concatenation, templates | Interpolation only (no `+`) |

## Mappings

### Types and Interfaces

**TypeScript:**
```typescript
interface User {
  id: number;
  name: string;
  email?: string;
}
```

**AIVI:**
```aivi
User = {
  id: Int
  name: Text
  email: Option Text
}
```

### Functions

**TypeScript:**
```typescript
const add = (x: number, y: number) => x + y;
```

**AIVI:**
```aivi
add = x y => x + y
```

Currying is default in AIVI. `add 1` returns a function `y => 1 + y`.

### Destructuring

**TypeScript:**
```typescript
const { name } = user;
```

**AIVI:**
```aivi
{ name } = user
```

### Patching / Spreading

**TypeScript:**
```typescript
const updated = {
  ...user,
  name: "Grace",
  stats: {
    ...user.stats,
    loginCount: user.stats.loginCount + 1
  }
};
```

**AIVI:**
```aivi
updated = user <= {
  name: "Grace"
  stats.loginCount: _ + 1
}
```

AIVI's deep patching is much more concise and doesn't require manual shallow copying at every level.

## The "Missing" Features

You might look for:
* **Classes with `this`**: AIVI has classes, but they are for logic reuse (traits/typeclasses), not for bundling state and behavior. State is separate data.
* **`any`**: There is no escape hatch. If you need dynamic data, you model it (e.g. valid JSON value).
