import { HashRouter as Router, Routes, Route, useLocation, Navigate } from 'react-router-dom';
import { AnimatePresence } from 'framer-motion';

// Pages
import LandingPage from '@/pages/LandingPage';
import TermsPage from '@/pages/TermsPage';
import DetectPage from '@/pages/DetectPage';
import GamePage from '@/pages/GamePage';
import SyntheticPage from '@/pages/SyntheticPage';
import BenchmarkPage from '@/pages/BenchmarkPage';
import ReviewPage from '@/pages/ReviewPage';
import SuccessPage from '@/pages/SuccessPage';
import ImportPage from '@/pages/ImportPage';
import CompanionPage from '@/pages/CompanionPage';
import FeedbackPage from '@/pages/FeedbackPage';

// Layout
import ContributeLayout from '@/components/ContributeLayout';

function AnimatedRoutes() {
  const location = useLocation();

  return (
    <AnimatePresence mode="wait">
      <Routes location={location} key={location.pathname}>
        <Route path="/" element={<LandingPage />} />
        <Route path="/terms" element={<TermsPage />} />
        <Route path="/detect" element={<Navigate to="/contribute/detect" replace />} />
        <Route path="/import" element={<ImportPage />} />
        <Route path="/companion" element={<CompanionPage />} />
        <Route path="/feedback" element={<FeedbackPage />} />
        <Route path="/success" element={<SuccessPage />} />
        
        {/* Contribute Flow with Layout */}
        <Route element={<ContributeLayout />}>
          <Route path="/contribute/detect" element={<DetectPage />} />
          <Route path="/contribute/synthetic" element={<SyntheticPage />} />
          <Route path="/contribute/game" element={<GamePage />} />
          <Route path="/contribute/benchmark" element={<BenchmarkPage />} />
          <Route path="/contribute/review" element={<ReviewPage />} />
        </Route>
      </Routes>
    </AnimatePresence>
  );
}

function App() {
  return (
    <Router>
      <div className="app-shell selection:bg-oracle selection:text-void">
        <AnimatedRoutes />
      </div>
    </Router>
  );
}

export default App;
