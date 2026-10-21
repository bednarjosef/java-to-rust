package library.items;

public class Book implements Item, Comparable<Book> {
    private final String title;
    private final int pages;

    public Book(String title, int pages) {
        this.title = title;
        this.pages = pages;
    }

    public String title() {
        return title;
    }

    public int pages() {
        return pages;
    }

    public int days() {
        return pages > 300 ? LOAN_DAYS * 2 : LOAN_DAYS;
    }

    public int compareTo(Book other) {
        return Integer.compare(pages, other.pages);
    }
}
