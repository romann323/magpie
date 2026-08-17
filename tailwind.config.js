/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        surface: {
          DEFAULT: '#0f1116',
          raised: '#171a22',
          hover: '#1e222c',
          border: '#2a2f3b',
        },
        accent: {
          DEFAULT: '#6366f1',
          hover: '#818cf8',
        },
        star: '#f5b400',
      },
      fontFamily: {
        sans: [
          'Inter',
          'system-ui',
          '-apple-system',
          'Segoe UI',
          'Roboto',
          'sans-serif',
        ],
      },
    },
  },
  plugins: [],
}
