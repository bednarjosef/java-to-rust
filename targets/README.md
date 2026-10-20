# Target programs

Typical Java programs the translator cannot translate yet. They are the end-to-end yardstick for
the object model (inheritance, interfaces, generics, casts), exceptions, inner classes and lambdas.

Each program is one file here. Alongside it:

- `Name.expected`: stdout from running the program with `java` (`java Name.java`).
- `Name.exitcode`: only when the exit code is not 0 (`UncaughtException` exits 1).
- `Name.input`: stdin for the programs that read with `Scanner` (`ScannerGrades`, `TextAdventure`).

Line 1 of each program is a `// Features:` comment listing what it needs. The table below repeats
those comments.

## Running

```
targets/run.sh                 # translate each file alone (single-file mode)
targets/run.sh JDK_ROOT        # translate whole programs, reading JDK classes from JDK_ROOT
```

`JDK_ROOT` is a source tree laid out by package (the `java.base` sources of a JDK). It is only
read, never copied. `run.sh` builds `java-to-rust`, translates each program into
`target/targets/<Name>/` as a Cargo crate, builds all crates into `target/targets/cargo`, runs
each binary with `Name.input` on stdin (or nothing), and compares stdout and the exit code. Each
program is reported as one of:

- `REJECTED`: the translator stopped with the message shown.
- `DOES NOT COMPILE`: the first rustc error of the generated crate.
- `WRONG OUTPUT`: stdout or the exit code differs from `Name.expected` / `Name.exitcode`.
- `PASS`.

## Current results

Recorded on main at be90ea9, before the object model, exceptions, records and lambdas:

| Mode | Summary |
|---|---|
| single-file (`targets/run.sh`) | 0 PASS, 0 WRONG OUTPUT, 0 DOES NOT COMPILE, 28 REJECTED, 28 programs |
| whole-program with a JDK `java.base` root (`targets/run.sh JDK_ROOT`) | 0 PASS, 0 WRONG OUTPUT, 0 DOES NOT COMPILE, 28 REJECTED, 28 programs |

In whole-program mode each program is given as a source directory of its own (javac rejects a
listed file next to a module's source root). The first blocker there:

| First blocker (whole-program) | Programs |
|---|---|
| a program interface, class tree or generic class | 17 |
| a record | 4 |
| a JDK interface (`java.io.Serializable` from collections, `java.util.List`) | 7 |

First blocker per program in single-file mode (the translator's own message):

| Program | First blocker |
|---|---|
| AnimalsInterfaces | interfaces not supported yet |
| BoundedGenerics | generic classes, inheritance and interfaces not supported yet |
| CheckedExceptions | generic classes, inheritance and interfaces not supported yet |
| ComparatorSorting | Object not supported yet |
| DequeBfs | exceptions not supported yet |
| DocumentVisitor | interfaces not supported yet |
| EnumAbstractMethods | interfaces not supported yet |
| EventListeners | generic classes, inheritance and interfaces not supported yet |
| ExpressionTree | generic classes, inheritance and interfaces not supported yet |
| GenericContainers | generic classes, inheritance and interfaces not supported yet |
| InnerClassIterator | generic classes, inheritance and interfaces not supported yet |
| IntegerKeyedGroups | type `Map<Integer,List<String>>` not supported yet |
| InventoryManagement | generic classes, inheritance and interfaces not supported yet |
| JsonPrettyPrinter | generic classes, inheritance and interfaces not supported yet |
| LambdaFunctions | generic methods not supported yet |
| ListOperations | type `List<?>` not supported yet |
| LruCacheLinkedHashMap | interfaces not supported yet |
| PriorityScheduler | generic classes, inheritance and interfaces not supported yet |
| RecordsAndSealed | record not supported yet |
| ScannerGrades | generic classes, inheritance and interfaces not supported yet |
| ShapesAbstract | generic classes, inheritance and interfaces not supported yet |
| StreamPipeline | record not supported yet |
| StringBuilderTable | record not supported yet |
| TextAdventure | type `Map<String,String>` not supported yet |
| TokenizerParser | record not supported yet |
| TryWithResources | generic classes, inheritance and interfaces not supported yet |
| UncaughtException | generic classes, inheritance and interfaces not supported yet |
| WordFrequency | type `Map<String,Integer>` not supported yet |

## Programs

| Program | Lines | Features |
|---|---|---|
| AnimalsInterfaces | 159 | Interfaces with default methods and static interface methods, a class implementing two interfaces, an abstract class implementing an interface, super.method() calls to a default method (Interface.super.method()), overriding, constants in an interface, instanceof with a pattern variable (including two patterns in one condition), a lambda for a functional interface with a default method, getClass().getSimpleName(), and for-each over a List of abstract types. |
| BoundedGenerics | 143 | Bounded type parameters <T extends Comparable<T>>, generic static methods, wildcards (List<? extends Number>, Comparable<? super T>), a user class implementing Comparable, generic max/min/clamp/binarySearch helpers, varargs generics, Number.doubleValue, Integer.parseInt. |
| CheckedExceptions | 156 | Custom checked exceptions (extends Exception) with a cause, custom unchecked exceptions (extends RuntimeException), throws clauses, try/catch/finally with finally ordering, multi-catch, rethrow, getCause, exception chaining, nested try blocks, return from finally overriding a return value, a finally block that runs on break, and catching a built-in NumberFormatException. |
| ComparatorSorting | 124 | A Person class with getters, Comparator.comparing with method references, thenComparing, reversed, nullsFirst, Comparator.comparingInt, an anonymous Comparator class, Collections.sort and List.sort, Arrays.sort on an Integer[] with a lambda, a stable-sort check, equals/hashCode on a user class used in a HashSet, and a binary search on a sorted list. |
| DequeBfs | 145 | ArrayDeque as a stack and as a queue (push, pop, offer, poll, peekFirst), breadth-first and depth-first search on a char grid, int[] pairs, 2D arrays, labelled loops, Deque iteration. |
| DocumentVisitor | 177 | The visitor pattern with a generic interface Visitor<R> and a generic method <R> R accept(Visitor<R>), several visitor implementations (rendering, counting, outline with an indent field), double dispatch through overloaded visit methods, a sealed-style hierarchy of nodes with a List of children, an anonymous Visitor class, and StringBuilder output. |
| EnumAbstractMethods | 178 | Enums with fields, constructors and abstract methods overridden per constant (constant bodies), a static lookup method over values(), valueOf with IllegalArgumentException, ordinal, compareTo, name, EnumMap and EnumSet, enum switch statements with fall-through, an enum implementing an interface, a static nested enum used as a HashMap key, and an enum-driven stack calculator. |
| EventListeners | 184 | A simple event system with a generic EventBus<E> holding Map<String, List<Listener<E>>> keyed by topic, a Listener functional interface, a listener interface with default methods, a named listener class and anonymous and lambda listeners, a sealed-free event hierarchy of abstract classes with subclasses, subscription handles with unsubscribe through an inner class that captures the bus, a dispatch loop that catches exceptions thrown by one listener and keeps going, and an ordered queue of pending events in an ArrayDeque. |
| ExpressionTree | 242 | An expression tree as an abstract class with subclasses (Num, Var, Add, Sub, Mul, Div, Neg), recursive eval over a Map<String, Integer> environment, a custom checked exception for undefined variables, an unchecked ArithmeticException for division by zero, toString with parentheses, simplification with instanceof checks and casts, and equals on the tree. |
| GenericContainers | 189 | User generic classes Pair<A, B>, Box<T>, and a generic linked Stack<T> with an inner Node class, generic methods with explicit and inferred type arguments, a custom unchecked exception on an empty stack, static nested generic class, equals and hashCode in user code. |
| IntegerKeyedGroups | 86 | HashMap with Integer keys and List values, computeIfAbsent, getOrDefault, HashSet of Integers, Set operations (retainAll, addAll), Integer boxing and unboxing, Integer.compare, Integer.MAX_VALUE, integer division and remainder. |
| InnerClassIterator | 181 | A user Iterable<T> implemented by a generic ring buffer with a non-static inner class iterator that reads the outer instance's fields (Ring.this), an anonymous Iterator in a method, a static nested Range class implementing Iterable<Integer>, a for-each over user Iterables, Iterator.remove on a user list, an inner class that shadows an outer field, a local class inside a method capturing an effectively final variable, and the Iterable.forEach default method with a lambda. |
| InventoryManagement | 167 | An inventory domain model with a checked custom exception and an unchecked one, HashMap<String, Item> keyed by SKU, TreeMap for sorted reports, ArrayList of transactions, a small Optional-returning lookup, String.format with width and precision, money in cents, enhanced for loops, switch on an enum of transaction kinds, and a summary report. |
| JsonPrettyPrinter | 245 | A JSON-like value model (an abstract JsonValue with JsonObject, JsonArray, JsonString, JsonNumber and JsonLiteral subclasses), polymorphic render with an indent parameter, StringBuilder with repeated spaces and escaping of quotes, LinkedHashMap to keep insertion order, a recursive descent reader over a String with a position field, a checked ParseException carrying the offset, instanceof patterns and casts, and String.join for the key list. |
| LambdaFunctions | 105 | java.util.function Function, BiFunction, Predicate, Supplier, Consumer, UnaryOperator, BinaryOperator, andThen, compose, negate, and, or, Function.identity, lambdas capturing effectively final locals, method references to static, instance and constructor targets (String::length, Integer::parseInt, StringBuilder::new, System.out::println), removeIf, replaceAll and forEach on a List, Map.forEach, and a user-written higher-order function. |
| ListOperations | 102 | ArrayList and LinkedList, List.of, add/remove/set/indexOf/subList, Iterator with remove, ListIterator, Collections.sort/reverse/max/min/frequency/swap/unmodifiableList, UnsupportedOperationException from an immutable list, toArray-free for-each loops. |
| LruCacheLinkedHashMap | 122 | A generic LruCache<K, V> that extends LinkedHashMap with access order and overrides removeEldestEntry (anonymous subclass constructor argument and a named subclass), a generic Pair holder, Map methods getOrDefault, computeIfAbsent, putIfAbsent, remove and containsKey, an eviction listener as a functional interface, an inner class counting hits and misses, a memoizing Fibonacci through the cache, and iteration over entrySet in access order. |
| PriorityScheduler | 112 | PriorityQueue with a Comparator lambda and with Comparator.comparingInt, a user Task class implementing Comparable, poll and peek, a simulated clock with a while loop, switch on a String, and String.format with width and zero padding. |
| RecordsAndSealed | 124 | Records with a compact canonical constructor that validates, extra methods and static factories on records, equals/hashCode/toString of records, records as HashMap keys, a sealed interface permitting records, a switch expression with type patterns and guards (when), record deconstruction patterns, nested records, and instanceof with a record pattern. |
| ScannerGrades | 123 | Scanner on System.in reading an int count, then nextLine for names, nextInt and nextDouble for scores, hasNextInt to skip bad tokens, a Student class with a grade method, an int[] histogram, Math.round and String.format with %.2f, a TreeMap of grade letters to names, a bad-input exception handled with try/catch, and a summary with averages and the best student. |
| ShapesAbstract | 167 | Abstract class with abstract and concrete methods, subclasses with super(...) calls, polymorphic dispatch through an array and a List, instanceof checks and casts, static counters in a class, protected fields, toString overrides, String.format with %.2f, Math.PI and Math.sqrt. |
| StreamPipeline | 100 | Stream and IntStream pipelines: filter, map, sorted, distinct, limit, skip, reduce, sum, average (OptionalDouble), count, anyMatch, allMatch, findFirst, mapToInt, range, boxed, collect with Collectors.toList, toSet, joining, groupingBy into a TreeMap with counting and summingInt, partitioningBy, toMap with a merge function, flatMap over nested lists. |
| StringBuilderTable | 115 | StringBuilder with append, insert, reverse, setCharAt, deleteCharAt, setLength and indexOf, String.format and printf with width, precision, left and right alignment, zero padding, hex and grouped integers, String methods repeat, strip, chars, toCharArray, compareTo, String.valueOf, text blocks, a text-wrapping routine, a box-drawing table renderer, and char arithmetic for a Caesar cipher. |
| TextAdventure | 148 | Scanner reading standard input line by line (`TextAdventure.input`), a Room class with exits in a HashMap<String, String>, an Item and an inventory in a List, a command dispatcher with a switch on the first word, a while loop until quit, a checked helper that throws, String methods split, trim, toLowerCase, startsWith and equals, a StringBuilder for the room description, and a final score line. |
| TokenizerParser | 205 | A tokenizer producing Token records from a String with a char-by-char loop, an enum of token kinds, a recursive-descent parser with one method per precedence level (let statements, expressions, terms, factors, unary minus, parentheses), a checked SyntaxException, a runtime EvalException for unknown names, a HashMap<String, Integer> environment, Character.isDigit and isLetter, Integer.parseInt, a while loop with index bookkeeping, and an interpreter loop over a program given as an array of lines. |
| TryWithResources | 134 | Try-with-resources on user AutoCloseable classes, close order (reverse of opening), suppressed exceptions from close() via getSuppressed, a resource opened in a try header with a catch and finally, nested resources, a resource that fails to open, a lambda AutoCloseable, and a generic helper that uses a resource supplier. |
| UncaughtException | 61 | A custom unchecked exception that escapes main, so the program exits with code 1 after printing; a state machine validating a sequence of commands, exception with a formatted message, an enum of states, a switch expression, and a finally block that still prints before the exception leaves the program. The expected exit code is 1 (see `UncaughtException.exitcode`). |
| WordFrequency | 84 | HashMap and TreeMap with String keys and Integer values, merge, getOrDefault, entrySet and for-each loops, String.split, text blocks, printf, Map.Entry, StringBuilder. |
