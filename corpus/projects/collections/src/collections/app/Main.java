package collections.app;

import collections.util.ArraySequence;
import collections.util.LinkedSequence;
import collections.util.Pair;
import collections.util.Sequence;
import collections.util.Sequences;

public class Main {
    enum Suit { CLUBS, DIAMONDS, HEARTS, SPADES }

    static final class Card implements Comparable<Card> {
        final int rank;
        final Suit suit;

        Card(int rank, Suit suit) {
            this.rank = rank;
            this.suit = suit;
        }

        @Override
        public int compareTo(Card o) {
            return rank != o.rank ? rank - o.rank : suit.compareTo(o.suit);
        }

        @Override
        public String toString() {
            return rank + " of " + suit;
        }
    }

    public static void main(String[] args) {
        ArraySequence<String> words = new ArraySequence<>();
        for (String w : "pear fig apple kiwi fig".split(" ")) {
            words.add(w);
        }
        Sequences.sort(words);
        System.out.println(Sequences.show(words) + " max " + Sequences.max(words) + " figs " + Sequences.count(words, "fig"));

        Sequence<Integer> squares = new LinkedSequence<>();
        for (int i = 1; i <= 5; i++) {
            squares.add(i * i);
        }
        int sum = 0;
        for (int i = 0; i < squares.size(); i++) {
            sum += squares.get(i);
        }
        System.out.println(Sequences.show(squares) + " sum " + sum + " max " + Sequences.max(squares) + " empty " + squares.isEmpty());

        ArraySequence<Card> hand = new ArraySequence<>();
        hand.add(new Card(10, Suit.HEARTS));
        hand.add(new Card(3, Suit.SPADES));
        hand.add(new Card(10, Suit.CLUBS));
        Sequences.sort(hand);
        System.out.println(Sequences.show(hand) + " best " + Sequences.max(hand));

        Sequence<Suit> suits = new LinkedSequence<>();
        suits.add(Suit.HEARTS);
        suits.add(Suit.CLUBS);
        System.out.println(Sequences.show(suits) + " " + Sequences.count(suits, Suit.CLUBS));

        Pair<String, Integer> p = new Pair<>("fig", 2);
        Pair<String, Suit> q = p.withSecond(Suit.SPADES);
        System.out.println(p + " " + q + " " + p.first().length() + " " + (p.second() + 1));
        System.out.println(p.equals(new Pair<>("fig", 2)) + " " + p.equals(q) + " " + (p.hashCode() == new Pair<>("fig", 2).hashCode()));
    }
}
