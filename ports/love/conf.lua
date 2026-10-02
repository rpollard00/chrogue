function love.conf(t)
  t.identity = 'chrogue-love'
  t.version = '11.5'
  t.window.title = 'Chrogue: LÖVE port of the battle'
  t.window.width = 1280
  t.window.height = 720
  t.window.minwidth = 640
  t.window.minheight = 360
  t.window.resizable = true
  t.window.vsync = 1
  t.modules.audio = false
  t.modules.joystick = false
  t.modules.physics = false
  t.modules.sound = false
  t.modules.touch = false
  t.modules.video = false
end
