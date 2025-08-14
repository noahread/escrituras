import React, { useState, useRef, useEffect } from 'react';
import { ChatMessage, Scripture } from '../types/scripture';
import { Send } from 'lucide-react';

interface ChatPanelProps {
  selectedScriptures: Scripture[];
}

const ChatPanel: React.FC<ChatPanelProps> = ({ selectedScriptures }) => {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [inputValue, setInputValue] = useState('');
  const [loading, setLoading] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  };

  useEffect(() => {
    scrollToBottom();
  }, [messages]);

  const sendMessage = async () => {
    if (!inputValue.trim() || loading) return;

    const userMessage: ChatMessage = {
      id: Date.now().toString(),
      role: 'user',
      content: inputValue,
      timestamp: new Date(),
      context: selectedScriptures
    };

    setMessages(prev => [...prev, userMessage]);
    setInputValue('');
    setLoading(true);

    try {
      // Use Tauri command instead of fetch
      const { invoke } = await import('@tauri-apps/api/tauri');
      
      const data = await invoke('chat_with_llm', {
        request: {
          message: inputValue,
          context: selectedScriptures,
          history: messages
        }
      });
      
      const assistantMessage: ChatMessage = {
        id: (Date.now() + 1).toString(),
        role: 'assistant',
        content: (data as any).response,
        timestamp: new Date()
      };

      setMessages(prev => [...prev, assistantMessage]);
    } catch (error) {
      console.error('Chat error:', error);
      const errorMessage: ChatMessage = {
        id: (Date.now() + 1).toString(),
        role: 'assistant',
        content: 'Sorry, I encountered an error. Please make sure you have configured an LLM provider.',
        timestamp: new Date()
      };
      setMessages(prev => [...prev, errorMessage]);
    } finally {
      setLoading(false);
    }
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  };

  return (
    <div className="chat-panel">
      <div style={{ padding: '1rem', borderBottom: '1px solid #e0e0e0', backgroundColor: '#f5f5f5' }}>
        <h3>Scripture Chat</h3>
        {selectedScriptures.length > 0 && (
          <div style={{ fontSize: '0.875rem', color: '#666', marginTop: '0.5rem' }}>
            Context: {selectedScriptures.length} scripture(s) selected
          </div>
        )}
      </div>
      
      <div className="chat-messages">
        {messages.length === 0 && (
          <div style={{ textAlign: 'center', color: '#666', padding: '2rem' }}>
            Start a conversation about the scriptures. Select verses from the left panel to add context.
          </div>
        )}
        
        {messages.map((message) => (
          <div key={message.id} className={`message ${message.role}`}>
            <div style={{ fontSize: '0.875rem', opacity: 0.7, marginBottom: '0.5rem' }}>
              {message.role === 'user' ? 'You' : 'Assistant'} • {message.timestamp.toLocaleTimeString()}
            </div>
            <div style={{ whiteSpace: 'pre-wrap' }}>{message.content}</div>
            {message.context && message.context.length > 0 && (
              <div style={{ fontSize: '0.75rem', opacity: 0.6, marginTop: '0.5rem' }}>
                Context: {message.context.map(s => s.verse_short_title).join(', ')}
              </div>
            )}
          </div>
        ))}
        
        {loading && (
          <div className="message assistant">
            <div style={{ fontSize: '0.875rem', opacity: 0.7, marginBottom: '0.5rem' }}>
              Assistant • thinking...
            </div>
            <div>...</div>
          </div>
        )}
        
        <div ref={messagesEndRef} />
      </div>
      
      <div className="chat-input">
        <textarea
          className="chat-input-field"
          placeholder="Ask a question about the scriptures..."
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          onKeyPress={handleKeyPress}
          disabled={loading}
        />
        <button 
          className="send-button"
          onClick={sendMessage}
          disabled={loading || !inputValue.trim()}
        >
          <Send size={16} style={{ marginRight: '0.5rem' }} />
          Send
        </button>
      </div>
    </div>
  );
};

export default ChatPanel;