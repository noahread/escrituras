export interface Scripture {
  volume_id: number;
  book_id: number;
  chapter_id: number;
  verse_id: number;
  volume_title: string;
  book_title: string;
  volume_long_title: string;
  book_long_title: string;
  volume_subtitle: string;
  book_subtitle: string;
  volume_short_title: string;
  book_short_title: string;
  volume_lds_url: string;
  book_lds_url: string;
  chapter_number: number;
  verse_number: number;
  scripture_text: string;
  verse_title: string;
  verse_short_title: string;
}

export interface Book {
  id: number;
  volume_id: number;
  book_title: string;
  book_long_title: string;
  book_subtitle: string;
  book_short_title: string;
  book_lds_url: string;
}

export interface Volume {
  id: number;
  volume_title: string;
  volume_long_title: string;
  volume_subtitle: string;
  volume_short_title: string;
  volume_lds_url: string;
}

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: Date;
  context?: Scripture[];
}