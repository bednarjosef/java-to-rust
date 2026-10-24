package marks;

import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;

/** An annotation that the library is compiled with; the program runs without it. */
@Retention(RetentionPolicy.CLASS)
public @interface CanIgnoreReturnValue {}
