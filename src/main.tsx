import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import App from './App'
import { MagnifierWindow } from './features/MagnifierWindow'
import './index.css'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 5_000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
})

// The magnifier popup is a separate Tauri window that loads the same
// bundle with a `#magnifier` hash so we can ship a single frontend
// artefact. Everything before the first `?` matters here.
const isMagnifierRoute =
  typeof window !== 'undefined' &&
  window.location.hash.replace(/^#/, '').split('?')[0] === 'magnifier'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      {isMagnifierRoute ? <MagnifierWindow /> : <App />}
    </QueryClientProvider>
  </StrictMode>
)
