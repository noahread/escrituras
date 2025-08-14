import React, { useState, useEffect } from 'react';
import { Scripture } from '../types/scripture';
import { scriptureDb } from '../utils/scriptureDb';
import { Search, Copy } from 'lucide-react';

interface ScripturePanelProps {
  onScriptureSelect: (scripture: Scripture) => void;
  selectedScriptures: Scripture[];
}

const ScripturePanel: React.FC<ScripturePanelProps> = ({ onScriptureSelect, selectedScriptures }) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<Scripture[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const initializeDb = async () => {
      try {
        await scriptureDb.initialize();
        setSearchResults(scriptureDb.searchScriptures('', 20));
        setLoading(false);
      } catch (err) {
        setError('Failed to load scriptures');
        setLoading(false);
      }
    };
    initializeDb();
  }, []);

  const handleSearch = (query: string) => {
    setSearchQuery(query);
    if (!loading && !error) {
      const results = scriptureDb.searchScriptures(query, 50);
      setSearchResults(results);
    }
  };

  const copyVerse = async (scripture: Scripture) => {
    const text = `${scripture.verse_title}\n${scripture.scripture_text}`;
    try {
      await navigator.clipboard.writeText(text);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  };

  const isSelected = (scripture: Scripture) => {
    return selectedScriptures.some(s => s.verse_id === scripture.verse_id);
  };

  if (loading) {
    return (
      <div className="scripture-panel">
        <div className="loading">Loading scriptures...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="scripture-panel">
        <div className="error">{error}</div>
      </div>
    );
  }

  return (
    <div className="scripture-panel">
      <div className="search-bar">
        <div style={{ position: 'relative' }}>
          <Search size={20} style={{ 
            position: 'absolute', 
            left: '12px', 
            top: '50%', 
            transform: 'translateY(-50%)',
            color: '#666'
          }} />
          <input
            type="text"
            className="search-input"
            placeholder="Search scriptures..."
            value={searchQuery}
            onChange={(e) => handleSearch(e.target.value)}
            style={{ paddingLeft: '40px' }}
          />
        </div>
      </div>
      
      <div className="scripture-content">
        {searchResults.map((scripture) => (
          <div 
            key={scripture.verse_id} 
            className={`verse ${isSelected(scripture) ? 'selected' : ''}`}
            style={{
              backgroundColor: isSelected(scripture) ? '#e3f2fd' : '#f9f9f9',
              borderLeftColor: isSelected(scripture) ? '#1976d2' : '#007acc'
            }}
          >
            <div className="verse-title" onClick={() => onScriptureSelect(scripture)}>
              {scripture.verse_title}
            </div>
            <div className="verse-text">{scripture.scripture_text}</div>
            <button
              onClick={() => copyVerse(scripture)}
              style={{
                background: 'none',
                border: 'none',
                cursor: 'pointer',
                marginTop: '8px',
                padding: '4px',
                borderRadius: '4px',
                display: 'flex',
                alignItems: 'center',
                gap: '4px',
                color: '#666'
              }}
              title="Copy verse"
            >
              <Copy size={16} />
              Copy
            </button>
          </div>
        ))}
        
        {searchResults.length === 0 && searchQuery && (
          <div style={{ textAlign: 'center', padding: '2rem', color: '#666' }}>
            No scriptures found for "{searchQuery}"
          </div>
        )}
      </div>
    </div>
  );
};

export default ScripturePanel;