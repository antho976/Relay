import { StatusBar } from 'expo-status-bar';
import { StyleSheet, Text, View, ScrollView } from 'react-native';

export default function App() {
  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <Text style={styles.brand}>◆ FORGE</Text>
      <Text style={styles.title}>Port in progress</Text>
      <Text style={styles.body}>
        The web React version of this app lives in {'\n'}
        <Text style={styles.code}>src/WorkoutTracker.web.jsx</Text>{'\n\n'}
        It needs to be ported to React Native syntax before the app will run.
        See <Text style={styles.code}>README.md</Text> for the porting checklist.
      </Text>
      <Text style={styles.body}>
        Once you've ported it, replace this file's contents with:{'\n\n'}
        <Text style={styles.code}>
          import WorkoutTracker from './src/WorkoutTracker';{'\n'}
          export default WorkoutTracker;
        </Text>
      </Text>
      <StatusBar style="light" />
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#1a1410' },
  content: { padding: 24, paddingTop: 80 },
  brand: { color: '#e85d4a', fontSize: 14, letterSpacing: 4, fontWeight: '700', marginBottom: 24 },
  title: { color: '#f5ead9', fontSize: 32, fontWeight: '700', marginBottom: 24 },
  body: { color: '#a89580', fontSize: 14, lineHeight: 22, marginBottom: 20 },
  code: { color: '#f5ead9', fontFamily: 'monospace', backgroundColor: '#241b15', paddingHorizontal: 4 },
});
