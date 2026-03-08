import { useEffect, useState } from "react"

/**
 * React hook for responsive design using CSS media queries
 * @param query - CSS media query string (e.g., "(max-width: 640px)")
 * @returns boolean - true if the media query matches
 *
 * @example
 * const isMobile = useMediaQuery("(max-width: 640px)")
 * const isTablet = useMediaQuery("(max-width: 1024px)")
 * const isDarkMode = useMediaQuery("(prefers-color-scheme: dark)")
 */
export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(false)

  useEffect(() => {
    const media = window.matchMedia(query)

    // Set initial value
    if (media.matches !== matches) {
      setMatches(media.matches)
    }

    // Create listener for changes
    const listener = () => setMatches(media.matches)
    media.addEventListener("change", listener)

    // Cleanup
    return () => media.removeEventListener("change", listener)
  }, [matches, query])

  return matches
}
