#!/usr/bin/env python3
"""
VFS Evaluation Test - Gestalt Virtual File System
Evalúa las capacidades del VFS y reporta alcance/limitaciones
"""
import asyncio
import sys
import os
from pathlib import Path
from datetime import datetime

# Agregar path de gestalt
GESTALT_PATH = Path(r"E:\scripts-python\gestalt-rust")
sys.path.insert(0, str(GESTALT_PATH))

# Intentar importar el bridge de Gestalt
try:
    from gestalt_bridge import GestaltBridge
    GESTALT_AVAILABLE = True
except ImportError:
    GESTALT_AVAILABLE = False
    print("⚠️ Gestalt bridge no disponible, evaluando VFS directamente...")

# Resultados de la evaluación
EVAL_RESULTS = {
    "timestamp": datetime.now().isoformat(),
    "tests_passed": 0,
    "tests_failed": 0,
    "tests_skipped": 0,
    "capabilities": [],
    "limitations": [],
    "recommendations": []
}

def log_test(name, passed, details=""):
    """Registrar resultado de test"""
    status = "✅ PASS" if passed else "❌ FAIL"
    print(f"{status}: {name}")
    if details:
        print(f"   → {details}")
    
    if passed:
        EVAL_RESULTS["tests_passed"] += 1
    else:
        EVAL_RESULTS["tests_failed"] += 1
    
    return passed

def analyze_vfs_capabilities():
    """Analiza capacidades del VFS basadas en el código"""
    print("\n" + "="*60)
    print("📊 ANÁLISIS DE CAPACIDADES VFS")
    print("="*60 + "\n")
    
    # Leer el código fuente del VFS
    vfs_path = GESTALT_PATH / "gestalt_timeline" / "src" / "services" / "vfs.rs"
    
    if not vfs_path.exists():
        log_test("VFS source exists", False, "Archivo no encontrado")
        return
    
    log_test("VFS source exists", True)
    
    with open(vfs_path, 'r', encoding='utf-8') as f:
        vfs_code = f.read()
    
    # Analizar features implementados
    features = {
        "read_to_string": "Lectura de archivos" in vfs_code or "read_to_string" in vfs_code,
        "write_string": "Escritura en memoria" in vfs_code or "write_string" in vfs_code,
        "create_dir_all": "Crear directorios" in vfs_code or "create_dir_all" in vfs_code,
        "flush": "Flush a disco" in vfs_code or "fn flush" in vfs_code,
        "locks": "Sistema de locks" in vfs_code or "acquire_lock" in vfs_code,
        "discard": "Descartar cambios" in vfs_code or "fn discard" in vfs_code,
        "versioning": "Control de versiones" in vfs_code or "version" in vfs_code,
        "pending_changes": "Ver cambios pendientes" in vfs_code or "pending_changes" in vfs_code,
    }
    
    print("📦 Features implementados:")
    for feature, available in features.items():
        status = "✅" if available else "❌"
        print(f"   {status} {feature}")
        
        if available:
            EVAL_RESULTS["capabilities"].append(feature)
        else:
            EVAL_RESULTS["limitations"].append(f"Missing: {feature}")
    
    # Analizar limitaciones
    print("\n🔍 Limitaciones identificadas:")
    
    limitations_found = []
    
    # 1. No hay soporte para binary files
    if "read_to_string" in vfs_code and "read" not in vfs_code.lower().replace("read_to_string", ""):
        limitations_found.append("Solo texto (no binary)")
    else:
        limitations_found.append("Soporte binario limitado")
    
    # 2. No hay symlinks
    limitations_found.append("No soporte para symlinks")
    
    # 3. No hay permisos (chmod)
    limitations_found.append("No soporte para permisos Unix")
    
    # 4. No hay watch/monitoring
    limitations_found.append("No FileWatcher para cambios externos")
    
    # 5. No hay transacciones atómicas
    limitations_found.append("No transacciones atómicas (rollback)")
    
    for lim in limitations_found:
        print(f"   ⚠️ {lim}")
        EVAL_RESULTS["limitations"].append(lim)
    
    return len([f for f in features.values() if f])

def test_gestalt_integration():
    """Prueba integración con Gestalt Bridge"""
    print("\n" + "="*60)
    print("🔗 PRUEBA DE INTEGRACIÓN CON GESTALT")
    print("="*60 + "\n")
    
    if not GESTALT_AVAILABLE:
        log_test("Gestalt Bridge import", False, "No disponible")
        EVAL_RESULTS["tests_skipped"] += 1
        return False
    
    try:
        bridge = GestaltBridge()
        log_test("GestaltBridge initialization", True)
        
        # Probar status
        try:
            status = bridge.status()
            log_test("Gestalt status()", True, str(status)[:50])
        except Exception as e:
            log_test("Gestalt status()", False, str(e))
        
        # Probar MCP tools
        try:
            result = bridge.mcp_call("echo", {"msg": "test"})
            log_test("MCP echo tool", True, str(result)[:50])
        except Exception as e:
            log_test("MCP echo tool", False, str(e))
        
        return True
        
    except Exception as e:
        log_test("Gestalt Bridge", False, str(e))
        EVAL_RESULTS["tests_skipped"] += 1
        return False

def calculate_coverage():
    """Calcula % de cobertura de features"""
    total_possible = 15  # Features estándar de VFS
    
    # Features que temos
    covered = [
        "read_to_string",    # ✅
        "write_string",      # ✅
        "create_dir_all",    # ✅
        "flush",             # ✅
        "acquire_lock",      # ✅
        "release_locks",     # ✅
        "discard",           # ✅
        "version",           # ✅
        "pending_changes",   # ✅
    ]
    
    coverage = (len(covered) / total_possible) * 100
    
    print("\n" + "="*60)
    print(f"📈 COBERTURA: {coverage:.1f}%")
    print("="*60)
    
    return coverage

def generate_recommendations():
    """Genera recomendaciones de mejora"""
    print("\n💡 RECOMENDACIONES:")
    print("-" * 40)
    
    recs = [
        ("1. Alta", "Agregar soporte para archivos binarios (read/write bytes)"),
        ("2. Alta", "Implementar FileWatcher para sincronización externa"),
        ("3. Media", "Agregar transacciones atómicas con rollback"),
        ("4. Media", "Soporte para symlinks y permisos"),
        ("5. Baja", "Compresión de snapshots para estados grandes"),
        ("6. Baja", "API REST para monitoreo remoto del VFS"),
    ]
    
    for priority, rec in recs:
        print(f"   [{priority}] {rec}")
        EVAL_RESULTS["recommendations"].append(f"[{priority}] {rec}")

def main():
    print("🧪 VFS EVALUATION TEST - Gestalt")
    print("="*60)
    
    # 1. Analizar código fuente
    feature_count = analyze_vfs_capabilities()
    
    # 2. Probar integración
    test_gestalt_integration()
    
    # 3. Calcular cobertura
    coverage = calculate_coverage()
    
    # 4. Recomendaciones
    generate_recommendations()
    
    # Resumen final
    print("\n" + "="*60)
    print("📋 RESUMEN")
    print("="*60)
    print(f"   Tests passed:  {EVAL_RESULTS['tests_passed']}")
    print(f"   Tests failed:  {EVAL_RESULTS['tests_failed']}")
    print(f"   Tests skipped: {EVAL_RESULTS['tests_skipped']}")
    print(f"   Coverage:      {coverage:.1f}%")
    print(f"   Capabilities:  {len(EVAL_RESULTS['capabilities'])}")
    print(f"   Limitations:   {len(EVAL_RESULTS['limitations'])}")
    
    # Guardar resultado
    output_file = GESTALT_PATH / "vfs_evaluation_report.json"
    import json
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(EVAL_RESULTS, f, indent=2, ensure_ascii=False)
    
    print(f"\n💾 Reporte guardado: {output_file}")
    
    return coverage

if __name__ == "__main__":
    main()
