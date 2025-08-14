import React, { useState } from 'react';
import ScripturePanel from './components/ScripturePanel';
import ChatPanel from './components/ChatPanel';
import { Scripture } from './types/scripture';

const App: React.FC = () => {
  const [selectedScriptures, setSelectedScriptures] = useState<Scripture[]>([]);

  const handleScriptureSelect = (scripture: Scripture) => {
    setSelectedScriptures(prev => {
      const exists = prev.some(s => s.verse_id === scripture.verse_id);
      if (exists) {
        return prev.filter(s => s.verse_id !== scripture.verse_id);
      } else {
        return [...prev, scripture];
      }
    });
  };

  return (
    <div className="app">
      <header style={{ 
        padding: '1rem 2rem', 
        backgroundColor: '#1976d2', 
        color: 'white',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center'
      }}>
        <h1 style={{ margin: 0, fontSize: '1.5rem' }}>Scripture Search & Chat</h1>
        <div style={{ fontSize: '0.875rem' }}>
          {selectedScriptures.length} verses selected
        </div>
      </header>
      
      <div className="main-content">
        <ScripturePanel 
          onScriptureSelect={handleScriptureSelect}
          selectedScriptures={selectedScriptures}
        />
        <ChatPanel selectedScriptures={selectedScriptures} />
      </div>
    </div>
  );
};

export default App;