// Features: a tokenizer producing Token records from a String with a char-by-char loop, an enum of
// token kinds, a recursive-descent parser with one method per precedence level (let statements,
// expressions, terms, factors, unary minus, parentheses), a checked SyntaxException, a runtime
// EvalException for unknown names, a HashMap<String, Integer> environment, Character.isDigit and
// isLetter, Integer.parseInt, a while loop with index bookkeeping, and an interpreter loop over a
// program given as an array of lines.
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class TokenizerParser {
    enum Kind {
        NUMBER, IDENT, PLUS, MINUS, STAR, SLASH, LPAREN, RPAREN, EQUALS, END
    }

    record Token(Kind kind, String text, int column) {
    }

    static class SyntaxException extends Exception {
        SyntaxException(String message) {
            super(message);
        }
    }

    static class EvalException extends RuntimeException {
        EvalException(String message) {
            super(message);
        }
    }

    static List<Token> tokenize(String line) throws SyntaxException {
        List<Token> tokens = new ArrayList<>();
        int i = 0;
        while (i < line.length()) {
            char c = line.charAt(i);
            if (Character.isWhitespace(c)) {
                i++;
                continue;
            }
            int start = i;
            if (Character.isDigit(c)) {
                while (i < line.length() && Character.isDigit(line.charAt(i))) {
                    i++;
                }
                tokens.add(new Token(Kind.NUMBER, line.substring(start, i), start));
                continue;
            }
            if (Character.isLetter(c)) {
                while (i < line.length() && Character.isLetterOrDigit(line.charAt(i))) {
                    i++;
                }
                tokens.add(new Token(Kind.IDENT, line.substring(start, i), start));
                continue;
            }
            Kind kind = switch (c) {
                case '+' -> Kind.PLUS;
                case '-' -> Kind.MINUS;
                case '*' -> Kind.STAR;
                case '/' -> Kind.SLASH;
                case '(' -> Kind.LPAREN;
                case ')' -> Kind.RPAREN;
                case '=' -> Kind.EQUALS;
                default -> throw new SyntaxException("bad character '" + c + "' at column " + i);
            };
            tokens.add(new Token(kind, String.valueOf(c), i));
            i++;
        }
        tokens.add(new Token(Kind.END, "", line.length()));
        return tokens;
    }

    static class Parser {
        private final List<Token> tokens;
        private final Map<String, Integer> env;
        private int index;

        Parser(List<Token> tokens, Map<String, Integer> env) {
            this.tokens = tokens;
            this.env = env;
        }

        private Token peek() {
            return tokens.get(index);
        }

        private Token advance() {
            return tokens.get(index++);
        }

        private boolean accept(Kind kind) {
            if (peek().kind() == kind) {
                index++;
                return true;
            }
            return false;
        }

        private void expect(Kind kind) throws SyntaxException {
            if (!accept(kind)) {
                throw new SyntaxException("expected " + kind + " but found " + describe(peek()));
            }
        }

        private static String describe(Token t) {
            return t.kind() == Kind.END ? "end of line" : "'" + t.text() + "'";
        }

        int statement() throws SyntaxException {
            if (peek().kind() == Kind.IDENT && tokens.get(index + 1).kind() == Kind.EQUALS) {
                String name = advance().text();
                advance();
                int value = expression();
                env.put(name, value);
                return value;
            }
            return expression();
        }

        int expression() throws SyntaxException {
            int value = term();
            while (peek().kind() == Kind.PLUS || peek().kind() == Kind.MINUS) {
                Kind op = advance().kind();
                int right = term();
                value = op == Kind.PLUS ? value + right : value - right;
            }
            return value;
        }

        int term() throws SyntaxException {
            int value = factor();
            while (peek().kind() == Kind.STAR || peek().kind() == Kind.SLASH) {
                Kind op = advance().kind();
                int right = factor();
                if (op == Kind.STAR) {
                    value *= right;
                } else {
                    if (right == 0) {
                        throw new EvalException("division by zero");
                    }
                    value /= right;
                }
            }
            return value;
        }

        int factor() throws SyntaxException {
            if (accept(Kind.MINUS)) {
                return -factor();
            }
            Token t = advance();
            switch (t.kind()) {
                case NUMBER:
                    return Integer.parseInt(t.text());
                case IDENT:
                    Integer value = env.get(t.text());
                    if (value == null) {
                        throw new EvalException("unknown variable " + t.text());
                    }
                    return value;
                case LPAREN:
                    int inner = expression();
                    expect(Kind.RPAREN);
                    return inner;
                default:
                    throw new SyntaxException("unexpected " + describe(t) + " at column " + t.column());
            }
        }
    }

    static String run(String line, Map<String, Integer> env) {
        try {
            Parser parser = new Parser(tokenize(line), env);
            int result = parser.statement();
            parser.expect(Kind.END);
            return String.valueOf(result);
        } catch (SyntaxException e) {
            return "syntax error: " + e.getMessage();
        } catch (EvalException e) {
            return "eval error: " + e.getMessage();
        }
    }

    public static void main(String[] args) throws SyntaxException {
        System.out.println(tokenize("x1 = (4+ 5)*2"));
        String[] program = {
            "x = 6 * 7",
            "y = x - 2 * (3 + 4)",
            "-y + x / 5",
            "z = (y - x) * -2",
            "z / (x - 42)",
            "w + 1",
            "3 +* 4",
            "(1 + 2",
            "10 / 3 # comment",
            "100 - 9 - 1",
            "2 * 3 * 4 - 5 / 2",
        };
        Map<String, Integer> env = new HashMap<>();
        for (String line : program) {
            System.out.println(line + "  =>  " + run(line, env));
        }
        System.out.println("environment: " + new java.util.TreeMap<>(env));
    }
}
