import { useCallback, useRef } from 'react';
import { useFocusEffect } from 'expo-router';

/** Ignore a completed action after navigating away or changing accounts. */
export function useActionScope(identity: string) {
  const state = useRef({ identity, generation: 0, focused: false });
  if (state.current.identity !== identity) {
    state.current.identity = identity;
    state.current.generation += 1;
  }
  useFocusEffect(useCallback(() => {
    state.current.focused = true;
    return () => {
      state.current.focused = false;
      state.current.generation += 1;
    };
  }, []));
  return useCallback(() => {
    const generation = state.current.generation;
    return () => state.current.focused && state.current.generation === generation;
  }, []);
}
