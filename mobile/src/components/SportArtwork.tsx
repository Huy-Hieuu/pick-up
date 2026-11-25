import React from 'react';
import { View } from 'react-native';
import type { Sport } from '../types/api';
import { Icon, palette } from './ui';

export function SportArtwork({ sport }: { sport: Sport }) {
  const pickleball = sport === 'pickleball';
  return (
    <View
      accessible={false}
      accessibilityElementsHidden
      importantForAccessibility="no-hide-descendants"
      style={[
        { width: '100%', height: '100%', pointerEvents: 'none', backgroundColor: pickleball ? '#E7EEDC' : '#F1EBD9', overflow: 'hidden', alignItems: 'center', justifyContent: 'center' },
      ]}
    >
      <View
        style={{
          position: 'absolute',
          width: '72%',
          height: '70%',
          left: '14%',
          top: '15%',
          borderWidth: 2,
          borderColor: pickleball ? '#A9BE94' : '#C7B98D',
          borderRadius: 18,
          opacity: 0.8,
        }}
      />
      <View
        style={{
          position: 'absolute',
          width: 2,
          height: '70%',
          left: '49.8%',
          top: '15%',
          backgroundColor: pickleball ? '#A9BE94' : '#C7B98D',
          opacity: 0.8,
        }}
      />
      <View
        style={{
          position: 'absolute',
          width: '18%',
          height: '18%',
          left: '41%',
          top: '41%',
          borderRadius: 999,
          borderWidth: 2,
          borderColor: pickleball ? '#A9BE94' : '#C7B98D',
          opacity: 0.8,
        }}
      />
      <View
        style={{
          width: 76,
          height: 76,
          borderRadius: 38,
          backgroundColor: palette.white,
          alignItems: 'center',
          justifyContent: 'center',
          boxShadow: "0 8px 18px rgba(25, 62, 50, 0.12)",
        }}
      >
        <Icon
          name={pickleball ? 'sun' : 'target'}
          color={palette.green}
          size={34}
        />
      </View>
      <View
        style={{
          position: 'absolute',
          right: '18%',
          bottom: '18%',
          width: 18,
          height: 18,
          borderRadius: 9,
          backgroundColor: pickleball ? palette.lime : '#D7C27A',
          borderWidth: 2,
          borderColor: palette.white,
        }}
      />
    </View>
  );
}
