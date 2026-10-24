package marks;

import java.lang.annotation.ElementType;
import java.lang.annotation.Target;

/** An annotation that the library is compiled with; the program runs without it. */
@Target({ElementType.TYPE_USE, ElementType.TYPE_PARAMETER})
public @interface Nullness {}
