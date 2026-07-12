import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import './styles.css';
import './media.css';

async function prepareDevRuntime() {
  if (import.meta.env.DEV && !('__TAURI_INTERNALS__' in window)) {
    const { installDevTauriMock } = await import('./devTauriMock');
    installDevTauriMock();
  }
}

void prepareDevRuntime().then(() => {
  ReactDOM.createRoot(document.getElementById('root')!).render(<React.StrictMode><App /></React.StrictMode>);
});
