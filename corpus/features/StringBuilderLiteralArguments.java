public class StringBuilderLiteralArguments {
    public static void main(String[] args) {
        StringBuilder sb = new StringBuilder();
        sb.insert(0, 4294967296L);
        sb.append(9223372036854775807L).append(' ').append(1.5f).append(2.5).append(-(3)).append((short) 7);
        sb.insert(0, 0.1f).insert(0, (byte) -3).insert(0, 'x').insert(0, 12345678901L * 2);
        System.out.println(sb);
        System.out.println(String.valueOf(4294967296L) + String.valueOf(0.1f) + String.valueOf((byte) 5));
    }
}
