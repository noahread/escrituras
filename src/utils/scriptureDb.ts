import { Scripture, Book, Volume } from '../types/scripture';

class ScriptureDatabase {
  private db: any = null;
  private scriptures: Scripture[] = [];
  private books: Book[] = [];
  private volumes: Volume[] = [];

  async initialize() {
    try {
      const response = await fetch('/lds-scriptures-2020.12.08/json/lds-scriptures-json.txt');
      const jsonText = await response.text();
      const data = JSON.parse(jsonText);
      
      this.volumes = data.volumes || [];
      this.books = data.books || [];
      this.scriptures = data.scriptures || [];
      
      console.log(`Loaded ${this.scriptures.length} scriptures`);
    } catch (error) {
      console.error('Failed to load scripture data:', error);
      throw error;
    }
  }

  searchScriptures(query: string, limit = 50): Scripture[] {
    if (!query.trim()) return this.scriptures.slice(0, limit);
    
    const searchTerm = query.toLowerCase();
    return this.scriptures
      .filter(scripture => 
        scripture.scripture_text.toLowerCase().includes(searchTerm) ||
        scripture.verse_title.toLowerCase().includes(searchTerm) ||
        scripture.book_title.toLowerCase().includes(searchTerm)
      )
      .slice(0, limit);
  }

  getScripturesByBook(bookTitle: string): Scripture[] {
    return this.scriptures.filter(scripture => 
      scripture.book_title.toLowerCase() === bookTitle.toLowerCase()
    );
  }

  getBooks(): Book[] {
    return this.books;
  }

  getVolumes(): Volume[] {
    return this.volumes;
  }

  getAllScriptures(): Scripture[] {
    return this.scriptures;
  }
}

export const scriptureDb = new ScriptureDatabase();