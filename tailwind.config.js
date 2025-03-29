module.exports = {
  content: {
    files: ["*.html", "./src/**/*.rs"],
  },
  theme: {
    extend: {
      colors: {
        primary: "#3B82F6",
        secondary: "#1E3A8A",
        dark: "#111827",
      },
      animation: {
        fadeIn: "fadeIn 1.5s ease-in-out",
        fadeOut: "fadeOut 1.5s ease-in-out",
        bounce: "bounce 2s infinite",
      },
      keyframes: {
        fadeIn: {
          "0%": { opacity: 0 },
          "100%": { opacity: 1 },
        },
        fadeOut: {
          "0%": { opacity: 1 },
          "100%": { opacity: 0 },
        },
        bounce: {
          "0%, 100%": {
            transform: "translateY(0)",
            animationTimingFunction: "cubic-bezier(0.8, 0, 1, 1)",
          },
          "50%": {
            transform: "translateY(25%)",
            animationTimingFunction: "cubic-bezier(0, 0, 0.2, 1)",
          },
        },
      },
    },
  },
  safelist: [
    'opacity-0',
    'opacity-100',
    'pointer-events-none',
    'pointer-events-auto',
    'rotate-180'
  ],
  plugins: [],
}
